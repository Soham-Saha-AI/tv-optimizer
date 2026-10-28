//! Optional source build on the PC. No TV mutation is performed by this module.
use crate::{actions::Engine,models::OperationProgress,transport::now_ms};
use serde::{Serialize,Deserialize};use sha2::{Digest,Sha256};
use std::{fs,io::Write,path::{Path,PathBuf},process::{Command,Stdio},sync::atomic::Ordering,thread,time::{Duration,Instant}};
#[derive(Clone,Debug,Serialize,Deserialize)]
pub struct OriginalStatus {pub ready:bool,pub terms_accepted:bool,pub version_name:String,pub version_code:u64,pub minimum_sdk:u32,pub message:String}
pub fn original_path(root:&Path)->Result<(PathBuf,OriginalStatus),String>{
 let raw=fs::read(root.join("data/launchers/original-manifest.json")).map_err(|_|"Get the latest CleanYourTV launcher first. Its source is built and checked on this PC.")?;
 // Windows PowerShell 5.1 writes a UTF-8 BOM; PowerShell 7 does not.
 let meta:serde_json::Value=serde_json::from_slice(raw.strip_prefix(&[0xef,0xbb,0xbf]).unwrap_or(&raw)).map_err(|_|"The prepared launcher record is invalid")?;
 if meta["schemaVersion"]!=1||meta["source"]!="https://github.com/Gathin23/cleanyourtv"||meta["applicationId"]!="com.gathin.tvhome.original"{return Err("The launcher source or package identity did not match.".into());}
 let revision=meta["revision"].as_str().ok_or("Missing source revision")?;let hash=meta["sha256"].as_str().ok_or("Missing APK integrity record")?;let signer=meta["signerSha256"].as_str().ok_or("Missing signing verification record")?;
 if revision.len()!=40||!revision.bytes().all(|b|b.is_ascii_hexdigit())||[hash,signer].iter().any(|h|h.len()!=64||!h.bytes().all(|b|b.is_ascii_hexdigit())){return Err("The prepared launcher verification record is incomplete.".into());}
 let path=root.join("data/launchers/original.apk");let length=fs::metadata(&path).map_err(|_|"Prepared APK is missing")?.len();if !(1000..=50*1024*1024).contains(&length){return Err("Prepared APK size was unexpected.".into());}
 if format!("{:x}",Sha256::digest(fs::read(&path).map_err(|e|e.to_string())?))!=hash{return Err("The prepared launcher was changed after verification. Get it again before installing.".into());}
 let minimum_sdk=meta["minimumAndroidSdk"].as_u64().filter(|n|*n>=21&&*n<=100).ok_or("Missing Android compatibility check")? as u32;
 Ok((path,OriginalStatus{ready:true,terms_accepted:root.join("data/original-builder/sdk-terms-accepted.json").is_file(),version_name:meta["versionName"].as_str().unwrap_or("unknown").into(),version_code:meta["versionCode"].as_u64().ok_or("Missing version check")?,minimum_sdk,message:"Official source built locally. Package identity, Android requirements, SHA-256 and APK signature were checked. Installation and preferred Home are separate choices.".into()}))
}
pub fn status(root:&Path)->OriginalStatus{original_path(root).map(|(_,s)|s).unwrap_or_else(|e|OriginalStatus{ready:false,terms_accepted:root.join("data/original-builder/sdk-terms-accepted.json").is_file(),version_name:String::new(),version_code:0,minimum_sdk:26,message:e})}
pub fn prepare(e:&Engine,accepted:bool,progress:impl Fn(OperationProgress))->Result<OriginalStatus,String>{
 let root=&e.root;let work=root.join("data/original-builder");fs::create_dir_all(&work).map_err(|e|e.to_string())?;
 if !accepted&&!work.join("sdk-terms-accepted.json").is_file(){return Err("Review and accept the Android SDK license in the launcher card before its optional tools are downloaded.".into());}
 let shell=PathBuf::from(std::env::var_os("SystemRoot").ok_or("Windows system folder unavailable")?).join("System32/WindowsPowerShell/v1.0/powershell.exe");
 let log=fs::File::create(work.join("build.log")).map_err(|e|e.to_string())?;
 // Execute this application's reviewed, compiled-in helper as commands. No execution-policy
 // setting is changed, and neither a frontend string nor a downloaded shell script is invoked.
 let mut command=Command::new(shell);command.args(["-NoLogo","-NoProfile","-NonInteractive","-Command","& ([scriptblock]::Create([Console]::In.ReadToEnd())) -Root $env:TVOPT_BUILD_ROOT -AcceptSdkTerms:($env:TVOPT_ACCEPT_SDK -eq '1')"]).env("TVOPT_BUILD_ROOT",root).env("TVOPT_ACCEPT_SDK",if accepted{"1"}else{"0"}).stdin(Stdio::piped()).stdout(Stdio::from(log.try_clone().map_err(|e|e.to_string())?)).stderr(Stdio::from(log));
 #[cfg(windows)]{use std::os::windows::process::CommandExt;command.creation_flags(0x08000000);}
 e.cancelled.store(false,Ordering::SeqCst);let mut child=command.spawn().map_err(|e|format!("Could not start the background launcher preparation: {e}"))?;
 if let Some(mut input)=child.stdin.take(){input.write_all(include_bytes!("../../scripts/Get-OriginalLauncher.ps1")).map_err(|e|format!("Could not send the launcher preparation steps: {e}"))?;}
 let started=Instant::now();let mut last=String::new();
 loop{
  if let Ok(text)=fs::read_to_string(work.join("progress.json")){if text!=last{if let Ok(v)=serde_json::from_str::<serde_json::Value>(text.trim_start_matches('\u{feff}')){progress(OperationProgress{phase:"preparing_launcher".into(),message:v["message"].as_str().unwrap_or("Preparing CleanYourTV").into(),completed:v["completed"].as_u64().unwrap_or(0).min(100)as usize,total:100,change_id:None});}last=text;}}
  if let Some(exit)=child.try_wait().map_err(|e|e.to_string())?{if !exit.success(){return Err(format!("CleanYourTV preparation stopped before installation. {} Details are saved in data/original-builder/build.log.",last));}break;}
  if e.cancelled.load(Ordering::SeqCst)||started.elapsed()>Duration::from_secs(3600){
   let mut stop=Command::new("taskkill.exe");stop.args(["/PID",&child.id().to_string(),"/T","/F"]).stdout(Stdio::null()).stderr(Stdio::null());#[cfg(windows)]{use std::os::windows::process::CommandExt;stop.creation_flags(0x08000000);}let _=stop.status();let _=child.kill();let _=child.wait();return Err("Preparation stopped. No app or TV setting was changed. Downloaded components can be reused next time.".into());
  }thread::sleep(Duration::from_millis(400));
 }
 let (_,result)=original_path(root)?;let _=fs::write(work.join("last-completed.txt"),now_ms().to_string());Ok(result)
}
#[cfg(test)]mod tests{use super::*;#[test]fn unprepared_or_untrusted_launcher_is_never_installable(){let root=std::env::temp_dir().join(format!("tvopt-builder-{}",uuid::Uuid::new_v4()));fs::create_dir_all(root.join("data/launchers")).unwrap();assert!(!status(&root).ready);fs::write(root.join("data/launchers/original-manifest.json"),br#"{"schemaVersion":1,"source":"https://untrusted.test","applicationId":"com.gathin.tvhome.original"}"#).unwrap();assert!(original_path(&root).is_err());let _=fs::remove_dir_all(root);}}
