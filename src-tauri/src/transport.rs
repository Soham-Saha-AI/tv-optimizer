//! A small ADB transport. Only backend code can construct command arguments.
use crate::models::DeviceRef;
use std::{collections::BTreeMap, io::Read, net::{IpAddr,Ipv4Addr,SocketAddr,TcpStream,UdpSocket}, path::{Path, PathBuf}, process::{Command, Stdio}, sync::{Arc,atomic::{AtomicUsize,Ordering},Mutex}, thread, time::{Duration, Instant, SystemTime, UNIX_EPOCH}};

#[derive(Clone)]
pub struct Adb { executable: Arc<PathBuf>, timeout: Duration }

impl Adb {
    pub fn new(root: PathBuf) -> Result<Self, String> {
        let name = if cfg!(windows) { "adb.exe" } else { "adb" };
        let candidates = [root.join("tools").join(name), root.join("tools/platform-tools").join(name), root.join("../platform-tools").join(name)];
        let executable = candidates.into_iter().find(|p| p.is_file()).ok_or_else(|| "ADB is missing. Extract the complete portable package, including its tools folder.".to_string())?;
        Ok(Self { executable: Arc::new(executable), timeout: Duration::from_secs(35) })
    }

    pub fn with_timeout(&self, seconds: u64) -> Self { Self { executable: self.executable.clone(), timeout: Duration::from_secs(seconds.clamp(1, 300)) } }

    pub fn run(&self, args: &[&str]) -> Result<String, String> {
        let mut command = Command::new(self.executable.as_ref());
        command.args(args).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
        #[cfg(windows)] { use std::os::windows::process::CommandExt; command.creation_flags(0x08000000); }
        let mut child = command.spawn().map_err(|e| format!("Could not start bundled ADB: {e}"))?;
        let stdout = child.stdout.take().ok_or("ADB output pipe was unavailable")?;
        let stderr = child.stderr.take().ok_or("ADB error pipe was unavailable")?;
        // Drain both streams concurrently: large package dumps must never fill a pipe.
        let out_thread = thread::spawn(move || read_capped(stdout));
        let err_thread = thread::spawn(move || read_capped(stderr));
        let started = Instant::now();
        let status = loop {
            match child.try_wait() {
                Ok(Some(status)) => break Ok(status),
                Ok(None) if started.elapsed() < self.timeout => thread::sleep(Duration::from_millis(30)),
                Ok(None) => { let _ = child.kill(); let _ = child.wait(); break Err("The TV did not reply in time. Reconnect and scan again before retrying a change.".to_string()); },
                Err(e) => { let _ = child.kill(); let _ = child.wait(); break Err(format!("Could not wait for ADB: {e}")); }
            }
        };
        let out = out_thread.join().map_err(|_| "Could not read ADB output")??;
        let err = err_thread.join().map_err(|_| "Could not read ADB errors")??;
        let status = status?;
        if !status.success() {
            return Err(format!("ADB could not complete the request: {}", summarize(if err.trim().is_empty() { &out } else { &err })));
        }
        Ok(out.trim_end_matches(['\r', '\n']).to_string())
    }

    pub fn shell(&self, serial: &str, command: &str) -> Result<String, String> {
        validate_serial(serial)?;
        if command.is_empty() || command.contains('\0') { return Err("Invalid internal TV operation".into()); }
        let output = self.run(&["-s", serial, "shell", command])?;
        if permission_failure(&output) { return Err(summarize(&output)); }
        Ok(output)
    }

    pub fn devices(&self) -> Result<Vec<DeviceRef>, String> { self.run(&["devices", "-l"]).map(|v| parse_devices(&v)) }

    pub fn connect(&self, address: &str) -> Result<String, String> {
        let address = normalize_address(address, true)?;
        let timeout=self.timeout.as_secs().min(15).max(1);
        let output = self.with_timeout(timeout).run(&["connect", &address])?;
        if ["failed", "unable", "cannot", "refused"].iter().any(|s| output.to_lowercase().contains(s)) { return Err(summarize(&output)); }
        Ok(output)
    }

    pub fn pair(&self, address: &str, code: &str) -> Result<String, String> {
        let address = normalize_address(address, false)?;
        if code.len() != 6 || !code.bytes().all(|b| b.is_ascii_digit()) { return Err("Enter the six-digit pairing code shown on the TV.".into()); }
        let output = self.with_timeout(20).run(&["pair", &address, code])?;
        if !output.to_lowercase().contains("successfully paired") { return Err(summarize(&output)); }
        Ok("TV paired. Use its connection address (a different port from the pairing address) to connect.".into())
    }

    pub fn discover(&self) -> Result<Vec<DeviceRef>, String> {
        // ADB's mDNS discovers modern wireless-debugging TVs without probing other LAN services.
        // Also probe the standard ADB port on a bounded private /24 for older Android TVs.
        let mut devices: BTreeMap<String, DeviceRef> = self.devices()?.into_iter().map(|d| (d.serial.clone(), d)).collect();
        if let Ok(output) = self.with_timeout(8).run(&["mdns", "services"]) {
            for line in output.lines().filter(|l| l.contains("_adb-tls-connect._tcp") || l.contains("_adb._tcp")).take(8) {
                if let Some(address) = line.split_whitespace().last() {
                    if normalize_address(address, false).is_ok() { let _ = self.with_timeout(5).connect(address); }
                }
            }
            if let Ok(updated) = self.devices() { for device in updated { devices.insert(device.serial.clone(), device); } }
        }
        for address in nearby_adb_addresses(){let _=self.with_timeout(3).connect(&address);}
        if let Ok(updated)=self.devices(){for device in updated{devices.insert(device.serial.clone(),device);}}
        Ok(devices.into_values().collect())
    }

    pub fn push(&self, serial: &str, local: &Path, remote: &str) -> Result<String, String> {
        validate_serial(serial)?; validate_remote_path(remote)?;
        let local = local.to_str().ok_or("Local file path is not valid Unicode")?;
        self.with_timeout(120).run(&["-s", serial, "push", local, remote])
    }

    pub fn pull(&self, serial: &str, remote: &str, local: &Path) -> Result<String, String> {
        validate_serial(serial)?; validate_remote_path(remote)?;
        let local = local.to_str().ok_or("Local file path is not valid Unicode")?;
        self.with_timeout(120).run(&["-s", serial, "pull", remote, local])
    }
}

fn local_subnet_hosts(local:Ipv4Addr)->Vec<SocketAddr>{
    if !local.is_private(){return vec![];}
    let [a,b,c,own]=local.octets();(1..=254).filter(|last|*last!=own).map(|last|SocketAddr::from((Ipv4Addr::new(a,b,c,last),5555))).collect()
}
fn nearby_adb_addresses()->Vec<String>{
    // UDP connect selects a local route without sending a datagram or contacting this address.
    // This deliberately scans at most one /24 and one port. Large/isolated networks use manual IP.
    let local=UdpSocket::bind("0.0.0.0:0").and_then(|s|{s.connect("192.0.2.1:80")?;s.local_addr()}).ok().and_then(|s|if let IpAddr::V4(ip)=s.ip(){Some(ip)}else{None});
    let Some(local)=local else{return vec![];};let hosts=local_subnet_hosts(local);let next=AtomicUsize::new(0);let found=Mutex::new(vec![]);
    thread::scope(|scope|{for _ in 0..24{let hosts=&hosts;let next=&next;let found=&found;scope.spawn(move||loop{let i=next.fetch_add(1,Ordering::Relaxed);let Some(address)=hosts.get(i)else{break;};if TcpStream::connect_timeout(address,Duration::from_millis(220)).is_ok(){let mut list=found.lock().unwrap();if list.len()<8{list.push(address.to_string());}}});}});
    found.into_inner().unwrap_or_default()
}

fn read_capped(mut reader: impl Read) -> Result<String, String> {
    const LIMIT: usize = 24 * 1024 * 1024;
    let mut bytes = Vec::new(); let mut buffer = [0u8; 8192]; let mut overflow = false;
    loop { let size = reader.read(&mut buffer).map_err(|e| e.to_string())?; if size == 0 { break; }
        let remaining = LIMIT.saturating_sub(bytes.len()); bytes.extend_from_slice(&buffer[..size.min(remaining)]); overflow |= size > remaining;
    }
    if overflow { return Err("TV inspection output exceeded its safety limit. The scan is incomplete.".into()); }
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

fn summarize(text: &str) -> String { text.trim().chars().take(900).collect() }
pub fn permission_failure(text: &str) -> bool {
    let lower = text.to_lowercase();
    ["permission denial", "permission denied", "securityexception", "exception occurred while executing", "error: device", "error: closed", "device offline", "device unauthorized"].iter().any(|p| lower.contains(p))
}
pub fn quote(value: &str) -> String { format!("'{}'", value.replace('\'', "'\\''")) }
pub fn validate_package(value: &str) -> bool {
    if value == "android" { return true; } // Android's framework package is the one standard undotted ID.
    value.len() <= 255 && value.contains('.') && value.split('.').all(|part| !part.is_empty() && part.bytes().enumerate().all(|(i,b)| if i == 0 { b.is_ascii_alphabetic() || b == b'_' } else { b.is_ascii_alphanumeric() || b == b'_' }))
}
pub fn validate_serial(value: &str) -> Result<(), String> {
    if value.is_empty() || value.len() > 256 || value.starts_with('-') || !value.bytes().all(|b| b.is_ascii_alphanumeric() || b"._:-[]%".contains(&b)) { return Err("Invalid TV connection identifier".into()); } Ok(())
}
fn validate_remote_path(value: &str) -> Result<(), String> {
    if !value.starts_with('/') || value.contains('\0') || value.contains('\n') || value.contains('\r') || value.split('/').any(|p| p == "..") { return Err("Invalid internal TV file path".into()); } Ok(())
}
fn normalize_address(value: &str, default_port: bool) -> Result<String, String> {
    let value = value.trim();
    if value.is_empty() || value.len() > 300 || value.starts_with('-') || value.bytes().any(|b| b.is_ascii_whitespace() || b"/\\'\";`$&|<>".contains(&b)) { return Err("Enter your TV's IP address, for example 192.0.2.10. The usual port is added automatically.".into()); }
    let (host, port) = if value.starts_with('[') {
        let (host, port) = value.split_once("]:").ok_or("IPv6 addresses need brackets and a port")?;
        let ip = host.trim_start_matches('['); if ip.parse::<IpAddr>().is_err() { return Err("Invalid IPv6 address".into()); }
        (format!("[{ip}]"), port.to_string())
    } else if let Some((host, port)) = value.rsplit_once(':') { (host.to_string(), port.to_string()) }
    else if default_port { (value.to_string(), "5555".to_string()) } else { return Err("Use the address and port shown on the TV.".into()); };
    if !host.starts_with('[') && (host.len() > 253 || !host.split('.').all(|s| !s.is_empty() && !s.starts_with('-') && !s.ends_with('-') && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-'))) { return Err("Invalid TV IP address or hostname".into()); }
    if !host.starts_with('[')&&host.bytes().all(|b|b.is_ascii_digit()||b==b'.')&&host.parse::<IpAddr>().is_err(){return Err("Invalid IPv4 address".into());}
    if port.parse::<u16>().ok().filter(|p| *p > 0).is_none() { return Err("The connection port must be between 1 and 65535.".into()); }
    Ok(format!("{host}:{port}"))
}
fn parse_devices(output: &str) -> Vec<DeviceRef> {
    output.lines().filter_map(|line| {
        let mut parts = line.split_whitespace(); let serial = parts.next()?; let state = parts.next()?;
        if serial == "List" || serial.starts_with('*') || !["device", "offline", "unauthorized", "recovery", "sideload", "no"].contains(&state) || validate_serial(serial).is_err() { return None; }
        let mut model = String::new(); let mut product = String::new();
        for field in parts { if let Some(value) = field.strip_prefix("model:") { model = value.replace('_', " "); } if let Some(value) = field.strip_prefix("product:") { product = value.to_string(); } }
        Some(DeviceRef { serial: serial.into(), state: if state == "no" { "no permissions".into() } else { state.into() }, model, product })
    }).collect()
}
pub fn now_ms() -> i64 { SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis().min(i64::MAX as u128) as i64 }

#[cfg(test)] mod tests {
    use super::*;
    #[test]fn same_network_discovery_is_bounded_and_only_uses_adb_port(){let hosts=local_subnet_hosts(Ipv4Addr::new(192,168,1,10));assert_eq!(hosts.len(),253);assert!(hosts.iter().all(|s|s.port()==5555&&s.ip().to_string().starts_with("192.168.1.")));assert!(!hosts.iter().any(|s|s.ip()==Ipv4Addr::new(192,168,1,10)));assert!(local_subnet_hosts(Ipv4Addr::new(8,8,8,8)).is_empty());}
    #[test]fn plain_ip_adds_port_but_pairing_never_guesses_it(){assert_eq!(normalize_address(" 192.0.2.10 ",true).unwrap(),"192.0.2.10:5555");assert_eq!(normalize_address("192.0.2.10:37123",true).unwrap(),"192.0.2.10:37123");assert!(normalize_address("192.0.2.10",false).is_err());for bad in ["999.168.1.2","192.168.1.2:xyz","192.168.1.2;reboot"]{assert!(normalize_address(bad,true).is_err());}}
    #[test] fn quote_cannot_escape_argument() { assert_eq!(quote("a'b;$(id)"), "'a'\\''b;$(id)'"); }
    #[test] fn rejects_package_injection() { for bad in ["x;id", "a.b --user 0", "a..b", ".a", "x.9bad"] { assert!(!validate_package(bad)); } assert!(validate_package("com.google.android.tvlauncher")); assert!(validate_package("android")); }
    #[test] fn validates_connect_and_pair_addresses() { assert_eq!(normalize_address("192.0.2.10", true).unwrap(), "192.0.2.10:5555"); assert_eq!(normalize_address("[fe80::1]:37001", false).unwrap(), "[fe80::1]:37001"); for bad in ["--help", "1.2.3.4:0", "1.2.3.4:65536", "localhost:55;id", "a b:55", "$(id):5555"] { assert!(normalize_address(bad, true).is_err()); } }
    #[test] fn parses_authorization_states_without_daemon_noise() { let d=parse_devices("* daemon started successfully\nList of devices attached\n192.0.2.10:5555 device product:mitv model:MiTV_AXSO1 transport_id:1\nUSB1 unauthorized\nUSB2 offline\n"); assert_eq!(d.len(),3); assert_eq!(d[0].model,"MiTV AXSO1"); assert_eq!(d[1].state,"unauthorized"); }
}
