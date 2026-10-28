//! Read-only TV inventory. Failed inspection is represented explicitly, never as absence.
use crate::{models::*, transport::{Adb, now_ms, validate_package}};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{collections::{BTreeMap, BTreeSet}, sync::OnceLock};

pub const PROFILE_ID: &str = "mitv-axso1-pi6157";
pub const GOOGLE_HOME: &str = "com.google.android.tvlauncher";
pub const PATCHWALL: &str = "com.mitv.tvhome.atv";
pub const PATCHWALL_LEGACY: &str = "com.mitv.tvhome";
pub const LAUNCHER: &str = "com.gathin.tvhome";
pub const ORIGINAL_LAUNCHER: &str = "com.gathin.tvhome.original";

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DeviceProfile { pub id:String, pub model:String, pub api:u32, pub build_marker:String, pub optional_packages:Vec<String>, pub launchers:Vec<String>, pub protected_packages:Vec<String> }
pub fn profile() -> &'static DeviceProfile { static PROFILE:OnceLock<DeviceProfile>=OnceLock::new();PROFILE.get_or_init(||serde_json::from_str(include_str!("../../profiles/mitv-axso1.json")).expect("bundled profile must be valid")) }
pub fn matches_profile(identity: &DeviceIdentity) -> bool {
    let p=profile(); identity.model.eq_ignore_ascii_case(&p.model) && identity.api == p.api && identity.fingerprint.split(|c:char|c.is_whitespace()||matches!(c,'='|'|'|'/'|':')).any(|token|token==p.build_marker)
}
pub fn allowed_optional(id: &str) -> bool { profile().optional_packages.iter().any(|p| p == id) }
pub fn is_protected(id: &str) -> bool {
    profile().protected_packages.iter().any(|p| p == id) || id == "android" || id.starts_with("com.android.input") || id.contains("inputmethod") || id.contains("keyboard") || id.contains("bluetooth") || id.contains("networkstack") || id.contains("permissioncontroller") || id.contains("packageinstaller") || id.contains("keychain") || id.contains("drm")
}
pub fn known_package(id: &str) -> (&str, &'static str) {
    match id {
        "android" => ("Android framework", "Essential Android system package; always protected."),
        GOOGLE_HOME => ("Google Android TV Home", "Google's original Home screen and its featured content."),
        PATCHWALL|PATCHWALL_LEGACY => ("Xiaomi PatchWall", "Xiaomi's content Home screen. Disabling it can affect the Mi remote button. Android preferred-Home support varies; this model uses a separate startup override."),
        LAUNCHER => ("SwiftOS", "Your custom Home screen. The selected signed build keeps native remote keys and makes Kolkata weather optional; compatible updates preserve preferences."),
        ORIGINAL_LAUNCHER => ("CleanYourTV original Home", "The original CleanYourTV launcher, installed separately so the custom Liquid Glass Home stays available."),
        "com.miui.tv.analytics" => ("Xiaomi TV analytics", "Usage analytics and reporting. Disabling stops this package's collection; no RAM gain is guaranteed."),
        "com.google.android.feedback" => ("Google feedback", "Feedback and crash-report submission. Sending feedback becomes unavailable while disabled."),
        "com.google.android.tv.bugreportsender" => ("TV bug report sender", "Manual TV bug report submission. Disabling prevents this report workflow."),
        "com.google.android.videos" => ("Google TV / Play Movies", "Purchased and rented Google films. Remove only if you do not use this app."),
        "com.google.android.youtube.tvmusic" => ("YouTube Music", "The separate TV music app; regular YouTube is a different package."),
        "com.google.android.youtube.tv" => ("YouTube", "Streaming playback. Protected from recommended removal."),
        "com.google.android.tvrecommendations" => ("Google TV recommendations", "Recommendation integration; no blanket removal rule is validated for this build."),
        "com.google.android.leanbacklauncher.recommendations" => ("Legacy Home recommendations", "Legacy Home recommendation integration; retained unless a validated control exists."),
        "com.android.tv.settings" => ("TV Settings and recovery Home", "Settings, inputs and Android's fallback Home must stay available."),
        "com.android.settings" => ("Android Settings", "Essential system settings and possible fallback Home."),
        "com.android.systemui" => ("Android system interface", "Essential menus, notifications and remote interactions."),
        "com.google.android.gms" => ("Google Play services", "Required by accounts, casting, voice, DRM and many streaming apps."),
        "com.google.android.gsf" => ("Google Services Framework", "Required Google account and app services."),
        "com.android.vending" => ("Google Play Store", "App installs and updates."),
        "com.google.android.katniss" => ("Google voice search", "Microphone and voice search integration."),
        "com.google.android.apps.mediashell" => ("Chromecast built-in", "Casting support."),
        "com.xiaomi.mitv.updateservice" => ("Xiaomi system updates", "Firmware updates; protected from removal."),
        "com.xiaomi.mitv.mediaexplorer" => ("Xiaomi media player", "USB and local-media playback."),
        "com.xiaomi.statistic" => ("Xiaomi statistics service", "Dependencies are not sufficiently verified; no removal recommendation."),
        "com.netflix.ninja" => ("Netflix", "Streaming playback and account sign-in."),
        "com.amazon.amazonvideo.livingroom" => ("Prime Video", "Amazon streaming playback and account sign-in."),
        "in.startv.hotstar" => ("Disney+ Hotstar", "Streaming playback and account sign-in."),
        "com.jio.media.ondemand" => ("JioCinema", "Streaming playback and account sign-in."),
        "com.disney.disneyplus" => ("Disney+", "Streaming playback and account sign-in."),
        "com.sonyliv" => ("Sony LIV", "Streaming playback and account sign-in."),
        "com.graymatrix.did" => ("ZEE5", "Streaming playback and account sign-in."),
        "com.spotify.tv.android" => ("Spotify", "Music playback and account sign-in."),
        "org.videolan.vlc" => ("VLC", "Local and network media player."),
        "org.xbmc.kodi" => ("Kodi", "Local and network media player with its own libraries and preferences."),
        "com.google.android.apps.youtube.kids" => ("YouTube Kids", "Children's streaming app and parental settings."),
        "com.plexapp.android" => ("Plex", "Personal media streaming and account sign-in."),
        "com.teamsmart.videomanager.tv" => ("SmartTube", "Third-party video app; inspect its source and account behavior separately."),
        "com.brave.browser" => ("Brave browser", "Web browsing and saved browser data."),
        _ => (id, "Purpose not yet verified. Review this app on the TV before changing it."),
    }
}

/// Short, reviewed purpose text, separate from compatibility and protection notes.
/// Package names alone are not evidence of an unknown service's purpose.
pub fn package_purpose(id: &str) -> &'static str {
    let (label, description)=known_package(id);
    if label==id { "Purpose not yet verified. Check this app on your TV before changing it." }
    else { description.split_inclusive('.').next().unwrap_or(description) }
}

pub fn existing_optimizations(s:&DeviceSnapshot)->Vec<String> {
    let mut notes=Vec::new();
    for p in &s.packages {
        if p.installed&&matches!(p.enabled,Some(2..=4)){notes.push(format!("{} was already disabled.",p.label));}
        else if !p.installed&&p.system&&allowed_optional(&p.id){notes.push(format!("{} was already removed for this TV user.",p.label));}
    }
    if s.settings.get("global/private_dns_mode").and_then(|v|v.as_deref())==Some("hostname") {
        if let Some(host)=s.settings.get("global/private_dns_specifier").and_then(|v|v.as_deref()) {
            if let Some((_,label,_))=crate::recommendations::DNS_PROVIDERS.iter().find(|(_,_,hostname)|*hostname==host){notes.push(format!("{label} DNS filtering was already configured."));}
        }
    }
    let keys=["global/window_animation_scale","global/transition_animation_scale","global/animator_duration_scale"];
    if keys.iter().all(|key|s.settings.get(*key).and_then(|v|v.as_deref()).and_then(|v|v.parse::<f32>().ok()).map(|v|(0.0..1.0).contains(&v)).unwrap_or(false)){notes.push("Reduced or disabled system animations were already configured.".into());}
    if let Some(home)=s.home_component.as_deref(){if [LAUNCHER,ORIGINAL_LAUNCHER].iter().any(|id|home.starts_with(&format!("{id}/"))){notes.push("A custom Home screen was already preferred.".into());}}
    notes
}

/// Small read-only identity probe, shared with photo-transfer guards.
pub(crate) fn read_identity(adb: &Adb, serial: &str) -> Result<(DeviceIdentity,String), String> {
    let props=parse_properties(&adb.shell(serial, "getprop")?);
    let property=|key:&str| props.get(key).cloned().unwrap_or_default();
    let model=property("ro.product.model");
    let api=property("ro.build.version.sdk").parse::<u32>().map_err(|_| "The TV did not provide its Android API version. Reconnect and authorize debugging.".to_string())?;
    if model.is_empty() { return Err("The TV did not provide its model. The scan cannot safely continue.".into()); }
    let user_output=adb.shell(serial,"am get-current-user")?;
    let user_id=user_output.trim().parse::<u32>().map_err(|_| "Could not establish the active TV user; package states cannot be interpreted safely.".to_string())?;
    let stable_serial=[property("ro.serialno"),property("ro.boot.serialno")].into_iter().find(|s|!s.is_empty()&&s!="unknown"&&s!="0123456789ABCDEF");
    let android_id=if stable_serial.is_none(){adb.shell(serial,&format!("settings --user {user_id} get secure android_id")).ok().and_then(|s|valid_android_id(s.trim()).then(||s.trim().to_string()))}else{None};
    let hardware_id=stable_serial.map(|s|format!("serial:{s}")).or_else(||android_id.map(|s|format!("android-id:{s}"))).unwrap_or_else(||format!("connection:{serial}"));
    let device_key=format!("{:x}", Sha256::digest(format!("{hardware_id}|{model}|user:{user_id}").as_bytes()));
    // Keep both vendor fingerprint and display ID. Xiaomi's PI.6157 compatibility marker is in display ID.
    let fingerprint=format!("{} | display={}", property("ro.build.fingerprint"), property("ro.build.display.id"));
    let identity=DeviceIdentity { serial:serial.into(), device_key, model, manufacturer:property("ro.product.manufacturer"), android_version:property("ro.build.version.release"), api, abi:property("ro.product.cpu.abi"), fingerprint, user_id };
    Ok((identity,hardware_id))
}

pub fn scan(adb: &Adb, serial: &str) -> Result<DeviceSnapshot, String> {
    let (identity,hardware_id)=read_identity(adb,serial)?;
    let user_id=identity.user_id;let api=identity.api;
    let mut inspections=vec![inspection("Device identity", "detected", format!("{} · Android {} · user {}",identity.model,identity.android_version,user_id))];
    if hardware_id.starts_with("connection:") { inspections.push(inspection("Stable TV identity", "could_not_check", "Hardware serial and Android device ID were unavailable. Inventory is tied to this connection address; applying and restoring changes is blocked to avoid targeting another TV at the same IP.")); }
    else {inspections.push(inspection("Stable TV identity","detected",if hardware_id.starts_with("android-id:"){ "A stable per-user Android device ID identifies this TV independently of its IP address." }else{"A hardware serial identifies this TV independently of its IP address."}));}
    let verified=matches_profile(&identity);
    let tv_features=adb.shell(serial,"pm list features").unwrap_or_default();
    let television=tv_features.lines().any(|line|line.trim()=="feature:android.software.leanback"||line.trim()=="feature:android.hardware.type.television");
    inspections.push(inspection("Android TV platform",if television{"detected"}else{"could_not_check"},if television{"The device advertises Android TV/Leanback features. Individual operations still require their own capability checks."}else{"Android TV platform features could not be verified. Inventory remains available; optimizations are unavailable until TV compatibility is established."}));
    inspections.push(inspection("Compatibility profile",if verified { "detected" } else { "needs_attention" },if verified { "MiTV-AXSO1 / Android 9 / PI.6157 profile matched. Exact package and launcher behavior has local evidence." } else { "No physically validated profile matches this firmware. Capability checks can enable standard settings and recognized optional-app controls; model-specific startup changes remain unavailable." }));
    // Capability probes are read-only. A recognized OS is insufficient evidence by itself.
    let shell_identity=adb.shell(serial,"id");
    let shell_privileged=shell_identity.as_ref().map(|out|out.contains("uid=2000(")||out.contains("uid=0(")).unwrap_or(false);
    // Android 9's package manager prints valid help but returns 255. Inspect its text without
    // interpreting that documented-help exit as a failure of all package capabilities.
    let pm_help=adb.shell(serial,"pm help; true").unwrap_or_default();
    let package_disable=shell_privileged&&pm_help.contains("disable-user")&&pm_help.contains("--user");
    let package_uninstall=shell_privileged&&pm_help.contains("uninstall")&&pm_help.contains("--user");
    let package_home=shell_privileged&&(pm_help.contains("set-home-activity")||pm_help.contains("set-home"));
    inspections.push(inspection("Package disable",if package_disable{"detected"}else{"could_not_check"},if package_disable{"ADB shell and per-user disable command are available. Only explicitly recognized optional packages are eligible; an untested model remains identified as such."}else{"Could not verify shell permissions and per-user disable support. App toggles remain unavailable."}));
    inspections.push(inspection("Package uninstall",if package_uninstall{"detected"}else{"could_not_check"},if package_uninstall{"Per-user uninstall is available. Eligibility, backup and recovery are checked separately for each app."}else{"Per-user uninstall support could not be verified."}));
    inspections.push(inspection("Home switch",if package_home&&api>=26{"detected"}else{"could_not_check"},if package_home&&api>=26{"Preferred-Home command is available. The current and target Home, fallback Home and recovery tools still require preflight before takeover."}else{"Preferred-Home switching could not be verified on this Android build."}));
    let recovery_tools=adb.shell(serial,"test -x /system/bin/nohup && test -x /system/bin/setsid && test -x /system/bin/sh && echo tvopt_recovery_tools").unwrap_or_default();
    inspections.push(inspection("Detached recovery",if recovery_tools.trim()=="tvopt_recovery_tools"{"detected"}else{"could_not_check"},"Read-only executable check for nohup, setsid and sh. Availability does not prove a timer survives disconnection; takeover performs a live preflight. TV power loss always stops the timer."));
    inspections.push(inspection("Remote input","needs_attention","Remote shortcuts and playback require physical checks. Software inventory cannot verify the real remote's held-volume, Mi, voice, HDMI or standby behavior. TV or remote firmware updates can add or change shortcuts absent from the factory manual; repeat physical checks after updates."));

    let installed_output=adb.shell(serial,&format!("pm list packages -f --user {user_id}"))?;
    let installed=parse_package_list(&installed_output);
    if installed.is_empty() { return Err("The installed-app inventory was empty or unreadable. It is unsafe to infer that this TV has no apps.".into()); }
    let all=match adb.shell(serial,&format!("pm list packages -f -u --user {user_id}")) {
        Ok(out) => { let parsed=parse_package_list(&out); if parsed.len()<installed.len() { inspections.push(inspection("Removed apps", "could_not_check", "The retained-package list was incomplete; only installed apps are shown.")); installed.clone() } else { parsed } },
        Err(error)=>{inspections.push(inspection("Removed apps", "could_not_check",error));installed.clone()}
    };
    let system_ids=match adb.shell(serial,&format!("pm list packages -s -u --user {user_id}")) { Ok(output)=>{let ids=parse_package_list(&output).into_keys().collect::<BTreeSet<_>>();if ids.is_empty(){inspections.push(inspection("System app classification","could_not_check","The system-app list was empty or unreadable. App origins are not assumed to be user-installed."));None}else{Some(ids)}}, Err(error)=>{inspections.push(inspection("System app classification","could_not_check",error));None} };
    let metadata=match adb.with_timeout(60).shell(serial,"dumpsys package packages") { Ok(output) => { let data=parse_package_dump(&output,user_id); if data.is_empty() { inspections.push(inspection("App state details","could_not_check","The TV did not expose per-user package metadata. App actions remain unavailable where state is unknown.")); } data }, Err(error)=>{inspections.push(inspection("App state details","could_not_check",error));BTreeMap::new()} };
    let query=format!("cmd package query-activities --brief --components --user {user_id} -a android.intent.action.MAIN -c android.intent.category.HOME");
    let launchers=match adb.shell(serial,&query) { Ok(output)=>{let components=parse_components(&output);inspections.push(inspection("Home activities",if components.is_empty(){"could_not_check"}else{"detected"},components.join("\n")));components.into_iter().filter_map(|s|s.split_once('/').map(|(p,_)|p.to_string())).collect::<BTreeSet<_>>()}, Err(error)=>{inspections.push(inspection("Home activities","could_not_check",error));BTreeSet::new()} };
    let resolve=format!("cmd package resolve-activity --brief --user {user_id} -a android.intent.action.MAIN -c android.intent.category.HOME");
    let home_component=match adb.shell(serial,&resolve) { Ok(output)=>{let found=parse_components(&output).pop(); inspections.push(inspection("Preferred Home",if found.is_some(){"detected"}else{"could_not_check"},found.clone().unwrap_or_else(||"Android did not report a resolved Home activity.".into())));found},Err(error)=>{inspections.push(inspection("Preferred Home","could_not_check",error));None} };
    let mut packages=Vec::new();
    for (id,path) in all {
        let detail=metadata.get(&id); let system=system_ids.as_ref().map(|s|s.contains(&id)).unwrap_or_else(||path.starts_with("/system/")||path.starts_with("/product/")||path.starts_with("/vendor/"));
        let classification_unknown=system_ids.is_none() && !system;
        let (label,reason)=known_package(&id);
        let mut paths=detail.map(|d|d.apk_paths.clone()).unwrap_or_default(); if path.ends_with(".apk")&&!paths.contains(&path){paths.insert(0,path);}
        let installed_state=installed.contains_key(&id); let enabled=detail.and_then(|d|d.enabled);
        let known_home=[GOOGLE_HOME,PATCHWALL,PATCHWALL_LEGACY,LAUNCHER,ORIGINAL_LAUNCHER,"com.google.android.leanbacklauncher"].contains(&id.as_str());
        let protected=is_protected(&id)||classification_unknown||(!allowed_optional(&id)&&!known_home&&system);
        let reason=if classification_unknown { "App origin could not be verified; changes are protected.".into() } else if enabled.is_none() { format!("{reason} Per-user enabled state could not be inspected; changes are unavailable.") } else { reason.into() };
        packages.push(PackageInfo{id:id.clone(),label:label.into(),installed:installed_state,enabled,system,version_name:detail.map(|d|d.version_name.clone()).unwrap_or_default(),version_code:detail.map(|d|d.version_code).unwrap_or_default(),apk_paths:paths,is_launcher:known_home||launchers.contains(&id),protected,reason,purpose:package_purpose(&id).into()});
    }
    packages.sort_by(|a,b|b.is_launcher.cmp(&a.is_launcher).then_with(||a.label.to_lowercase().cmp(&b.label.to_lowercase())));
    inspections.push(inspection("App inventory","detected",format!("{} installed, {} disabled, {} removed for user {}. Apps without readable states remain unknown.",packages.iter().filter(|p|p.installed).count(),packages.iter().filter(|p|p.installed&&matches!(p.enabled,Some(2..=4))).count(),packages.iter().filter(|p|!p.installed).count(),user_id)));

    let mut settings=BTreeMap::new();
    if user_id==0&&packages.iter().any(|p|p.id==LAUNCHER&&p.installed&&p.version_code>=24){match crate::wallpapers::read_mode(adb,serial){Ok((enabled,permission))=>{settings.insert("swiftos/auto_wallpaper".into(),Some(if enabled{"1"}else{"0"}.into()));inspections.push(inspection("Local wallpaper engine","detected","New decoder and indexing work starts only while Home is visible; in-flight work may finish after it is hidden."));inspections.push(inspection("Local wallpaper photo access",if permission{"detected"}else{"needs_attention"},if permission{"SwiftOS can read local photos."}else{"Allow SwiftOS photo access on the TV before enabling the local engine."}));},Err(e)=>inspections.push(inspection("Local wallpaper engine","could_not_check",e))}}
    for (namespace,keys) in [("global",vec!["window_animation_scale","transition_animation_scale","animator_duration_scale","private_dns_mode","private_dns_specifier","set_patchwall_default"]),("secure",vec!["enabled_accessibility_services","accessibility_enabled"])] {
        // Listing preserves the difference between an absent setting and an unreadable namespace.
        // Global settings are shared. Android 9 rejects --user for list; unqualified secure
        // reads are safe for user 0 only. Other users fail closed if their OS lacks this syntax.
        let list=if namespace=="global"||user_id==0{format!("settings list {namespace}")}else{format!("settings --user {user_id} list {namespace}")};
        match adb.shell(serial,&list) {
            Ok(output) if !looks_like_error(&output)&&output.lines().any(|l|l.contains('='))=>{let values=parse_settings(&output);for key in keys {settings.insert(format!("{namespace}/{key}"),values.get(key).cloned());}},
            Ok(output)=>inspections.push(inspection(&format!("{namespace} settings"),"could_not_check",output)),
            Err(error)=>inspections.push(inspection(&format!("{namespace} settings"),"could_not_check",error))
        }
    }
    let readable_global=settings.contains_key("global/window_animation_scale");
    inspections.push(inspection("Private DNS",if api>=28&&shell_privileged&&readable_global{"detected"}else{"could_not_check"},if api<28{"Encrypted Android Private DNS requires Android 9 / API 28 or newer. This TV needs a separately configured router or other network-level filter."}else if shell_privileged&&readable_global{"Android Private DNS and readable global settings are available. Applying a profile still verifies DNS activation and internet connectivity, with rollback on failure."}else{"Private DNS setting access could not be verified; DNS controls remain unavailable."}));
    inspections.push(inspection("Animations capability",if api>=17&&shell_privileged&&readable_global{"detected"}else{"could_not_check"},if api>=17&&shell_privileged&&readable_global{"Android animation scales are readable through an authorized shell. The apply engine verifies each write."}else{"Android animation settings could not be verified."}));
    let animation_keys=["window_animation_scale","transition_animation_scale","animator_duration_scale"];
    let animations=animation_keys.iter().map(|k|settings.get(&format!("global/{k}")).and_then(|v|v.as_deref()).unwrap_or("default")).collect::<Vec<_>>();
    inspections.push(inspection("Animations", if animation_keys.iter().all(|k|settings.contains_key(&format!("global/{k}"))) {if animations.iter().all(|v|*v=="0.5"||*v=="0"||*v=="0.0"){"already_optimized"}else{"detected"}}else{"could_not_check"},format!("Window / transition / animator scales: {}",animations.join(" / "))));
    if let Some(mode)=settings.get("global/private_dns_mode") {
        let provider=settings.get("global/private_dns_specifier").and_then(|v|v.as_deref()).unwrap_or("none");
        let recognized=crate::recommendations::dns_hostname(provider).is_some();
        inspections.push(inspection("DNS filtering",if mode.as_deref()==Some("hostname")&&recognized{"already_optimized"}else{"detected"},format!("Mode: {}; provider: {provider}. {} DNS cannot remove every sponsored card or in-stream ad.",mode.as_deref().unwrap_or("system default"),if recognized{"A bundled filtering provider is configured."}else{"Filtering by this provider has not been established."})));
    }
    if let Some(Some(services))=settings.get("secure/enabled_accessibility_services") { inspections.push(inspection("Remote accessibility helpers","detected",format!("Enabled services: {services}. The custom Home-key helper previously affected held-volume repeat on this TV."))); }
    let (memory_total_kb,memory_available_kb)=match adb.shell(serial,"cat /proc/meminfo") {Ok(output)=>parse_memory(&output),Err(error)=>{inspections.push(inspection("Memory","could_not_check",error));(None,None)}};
    let storage_free_kb=match adb.shell(serial,"df -k /data") { Ok(output)=>parse_storage(&output),Err(error)=>{inspections.push(inspection("Storage","could_not_check",error));None} };
    let processes=match adb.shell(serial,"ps -A -o PID,USER,RSS,NAME") {Ok(output) if process_header(&output)=>output,_=>match adb.shell(serial,"ps -A"){Ok(output)if process_header(&output)=>output,Ok(_)=>{inspections.push(inspection("Running processes","could_not_check","The TV did not return a readable process table."));String::new()},Err(error)=>{inspections.push(inspection("Running processes","could_not_check",error));String::new()}}};
    if memory_total_kb.is_none()||memory_available_kb.is_none(){inspections.push(inspection("Memory totals","could_not_check","The TV did not expose complete memory totals; no memory gain is estimated."));}
    if storage_free_kb.is_none(){inspections.push(inspection("Storage free space","could_not_check","The TV did not expose usable free-space information."));}
    if verified { inspections.push(inspection("Known remote routes","detected","Circle uses Android Home; custom builds recognize double Circle for App Library without accessibility interception. Mi routes to PatchWall. Physically check held volume, voice, HDMI, casting, standby and playback after changes.")); }
    let mut snapshot=DeviceSnapshot{identity,scanned_at:now_ms(),profile:verified.then(||PROFILE_ID.into()),packages,home_component,settings,memory_total_kb,memory_available_kb,storage_free_kb,processes,inspections};
    let existing=existing_optimizations(&snapshot);
    snapshot.inspections.push(inspection("Existing optimizations",if existing.is_empty(){"detected"}else{"already_optimized"},if existing.is_empty(){"No reviewed pre-applied optimization was detected in the readable checks. Unknown or inaccessible settings are not assumed to be original.".into()}else{format!("{}\nThese are observed current states; who applied them and their earlier values are unknown.",existing.join("\n"))}));
    Ok(snapshot)
}

fn inspection(name:&str,status:&str,detail:impl Into<String>)->Inspection{Inspection{name:name.into(),status:status.into(),detail:detail.into()}}
fn valid_android_id(value:&str)->bool{value.len()==16&&value.bytes().all(|b|b.is_ascii_hexdigit())&&value!="0000000000000000"&&value!="9774d56d682e549c"}
fn process_header(value:&str)->bool{!looks_like_error(value)&&value.lines().next().map(|l|l.split_whitespace().any(|v|v=="PID")).unwrap_or(false)}
fn looks_like_error(output:&str)->bool{let s=output.to_lowercase();s.starts_with("error")||s.starts_with("unknown command")||s.starts_with("usage:")||s.contains("not found")||crate::transport::permission_failure(output)}
fn parse_properties(output:&str)->BTreeMap<String,String>{output.lines().filter_map(|line|{let (k,v)=line.strip_prefix('[')?.split_once("]: [")?;Some((k.into(),v.strip_suffix(']')?.into()))}).collect()}
pub fn parse_package_list(output:&str)->BTreeMap<String,String>{output.lines().filter_map(|line|{let value=line.trim().strip_prefix("package:")?;let (path,id)=value.rsplit_once('=').unwrap_or(("",value));validate_package(id).then(||(id.into(),path.into()))}).collect()}
pub fn parse_components(output:&str)->Vec<String>{output.lines().filter_map(|line|{let value=line.trim();let(p,c)=value.split_once('/')?;if validate_package(p)&&!c.is_empty()&&c.bytes().all(|b|b.is_ascii_alphanumeric()||b"._$".contains(&b)){Some(value.into())}else{None}}).collect()}
pub fn parse_settings(output:&str)->BTreeMap<String,String>{output.lines().filter_map(|line|line.split_once('=').map(|(k,v)|(k.to_string(),v.to_string()))).collect()}
#[derive(Debug,Default,Clone)]struct PackageDetail{enabled:Option<u32>,version_name:String,version_code:u64,apk_paths:Vec<String>}
fn parse_package_dump(output:&str,user:u32)->BTreeMap<String,PackageDetail>{
    let mut result=BTreeMap::new();let mut current:Option<(String,PackageDetail)>=None;
    for line in output.lines(){let t=line.trim();if let Some(rest)=t.strip_prefix("Package ["){if let Some((id,d))=current.take(){result.entry(id).or_insert(d);}if let Some((id,_))=rest.split_once(']'){if validate_package(id){current=Some((id.into(),PackageDetail::default()));}}continue;}
        if let Some((_,detail))=current.as_mut(){
            if let Some(v)=t.strip_prefix("versionName="){detail.version_name=v.into();}
            if let Some(v)=t.strip_prefix("versionCode="){detail.version_code=v.split_whitespace().next().unwrap_or("").parse().unwrap_or(0);}
            if t.starts_with(&format!("User {user}:")){for token in t.split_whitespace(){if let Some(v)=token.strip_prefix("enabled="){detail.enabled=v.parse::<u32>().ok().filter(|n|*n<=4);}}}
            if let Some(v)=t.strip_prefix("codePath="){if v.ends_with(".apk"){detail.apk_paths.push(v.into());}}
            if let Some(v)=t.strip_prefix("splitCodePaths="){for p in v.trim_matches(['[',']']).split(',').map(str::trim){if p.starts_with('/')&&p.ends_with(".apk"){detail.apk_paths.push(p.into());}}}
        }
    }if let Some((id,d))=current{result.entry(id).or_insert(d);}result
}
fn parse_memory(output:&str)->(Option<u64>,Option<u64>){let values=output.lines().filter_map(|line|{let(k,v)=line.split_once(':')?;Some((k,v.split_whitespace().next()?.parse::<u64>().ok()?))}).collect::<BTreeMap<_,_>>();(values.get("MemTotal").copied(),values.get("MemAvailable").copied())}
fn parse_storage(output:&str)->Option<u64>{output.lines().filter_map(|line|{let fields=line.split_whitespace().collect::<Vec<_>>();if fields.len()>=6&&fields[0].starts_with('/') {fields[3].parse::<u64>().ok()}else{None}}).next()}

#[cfg(test)]mod tests{use super::*;
 #[test]fn purpose_notes_are_short_reviewed_and_unknown_stays_unknown(){assert_eq!(package_purpose("com.miui.tv.analytics"),"Usage analytics and reporting.");assert_eq!(package_purpose("com.google.android.feedback"),"Feedback and crash-report submission.");assert!(package_purpose("com.vendor.analytics.suggestive_name").starts_with("Purpose not yet verified"));for id in &profile().optional_packages{assert!(package_purpose(id).split_whitespace().count()<20);assert!(!package_purpose(id).contains("not yet verified"));}let old=r#"{"id":"com.example.old","label":"Old app","installed":true,"enabled":3,"system":false,"version_name":"1","version_code":1,"apk_paths":[],"is_launcher":false,"protected":false,"reason":"Old scan"}"#;assert!(serde_json::from_str::<PackageInfo>(old).unwrap().purpose.is_empty());}
 #[test]fn list_handles_paths_and_removed_packages(){let p=parse_package_list("package:/system/app/a/base.apk=com.example.a\npackage:com.example.b\nSecurityException\n");assert_eq!(p.len(),2);assert_eq!(p["com.example.a"],"/system/app/a/base.apk");}
 #[test]fn exact_user_and_default_enabled_state(){let p=parse_package_dump("Packages:\n  Package [com.example.a] (abc):\n    versionCode=21 minSdk=26\n    versionName=2.7\n    User 0: installed=true enabled=0\n    User 10: installed=false enabled=3\n  Package [com.example.b] (def):\n    User 0: installed=true enabled=3\n",0);assert_eq!(p["com.example.a"].enabled,Some(0));assert_eq!(p["com.example.a"].version_code,21);assert_eq!(p["com.example.b"].enabled,Some(3));}
 #[test]fn unreadable_state_remains_unknown(){assert_eq!(parse_package_dump("Package [com.example.a] (a):\n permission denied",0)["com.example.a"].enabled,None);}
 #[test]fn ignores_hidden_factory_duplicate(){let p=parse_package_dump("Package [com.example.a] (a):\nversionCode=21\nUser 0: enabled=3\nHidden system packages:\nPackage [com.example.a] (b):\nversionCode=1\nUser 0: enabled=0",0);assert_eq!(p["com.example.a"].version_code,21);assert_eq!(p["com.example.a"].enabled,Some(3));}
 #[test]fn absence_and_literal_null_differ(){let p=parse_settings("foo=null\nbar=\nkey=a=b\n");assert_eq!(p.get("foo").map(String::as_str),Some("null"));assert_eq!(p.get("bar").map(String::as_str),Some(""));assert!(!p.contains_key("missing"));assert_eq!(p["key"],"a=b");}
 #[test]fn component_parser_rejects_resolver_noise(){assert_eq!(parse_components("priority=2\ncom.gathin.tvhome/.MainActivity\nNo activity found\ncom.example.bad/Bad Activity\n"),vec!["com.gathin.tvhome/.MainActivity"]);}
 #[test]fn essential_packages_protected(){for p in ["com.android.tv.settings","com.android.systemui","com.google.android.gms","com.google.android.apps.mediashell","com.android.bluetooth","com.google.android.apps.inputmethod.hindi","com.vendor.keyboard"]{assert!(is_protected(p),"{p}");}assert!(!is_protected("com.miui.tv.analytics"));}
 #[test]fn parses_real_totals(){assert_eq!(parse_memory("MemTotal: 1000 kB\nMemAvailable: 400 kB"),(Some(1000),Some(400)));assert_eq!(parse_storage("Filesystem 1K-blocks Used Available Use% Mounted on\n/dev/block/dm-1 10000 3000 7000 30% /data"),Some(7000));}
 #[test]fn rejects_unstable_android_identifiers(){for id in ["null","unknown","0000000000000000","9774d56d682e549c","bad;id"]{assert!(!valid_android_id(id));}assert!(valid_android_id("5d5d9ca33c1ac7ff"));}
 #[test]fn firmware_match_does_not_accept_similar_build_numbers(){let mut i=DeviceIdentity{serial:"test".into(),device_key:"key".into(),model:"MiTV-AXSO1".into(),manufacturer:"Xiaomi".into(),android_version:"9".into(),api:28,abi:"armeabi-v7a".into(),fingerprint:"test | display=PI.61570 release-keys".into(),user_id:0};assert!(!matches_profile(&i));i.fingerprint="test | display=PI.6157 release-keys".into();assert!(matches_profile(&i));i.api=29;assert!(!matches_profile(&i));}
}
