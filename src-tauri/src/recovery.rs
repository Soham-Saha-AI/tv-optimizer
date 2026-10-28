//! Treat imported recovery JSON as data, never as a script or an unrestricted command source.
use crate::{actions,models::ChangeRecord,scanner,transport::validate_package};
use serde_json::Value;

pub fn validate_record(r:&ChangeRecord)->Result<(),String>{
    if uuid::Uuid::parse_str(&r.id).is_err()||uuid::Uuid::parse_str(&r.session_id).is_err(){return Err("Invalid recovery record identity".into());}
    if !["prepared","applied","confirmed","awaiting_confirmation","failed","uncertain"].contains(&r.status.as_str())||!["available","restored","not_available","needs_review"].contains(&r.restore_status.as_str()){return Err("Unknown recovery record status".into());}
    if r.before_json.len()>1_000_000||r.after_json.len()>1_000_000||r.target.len()>1024||r.label.len()>2048{return Err("Oversized recovery state".into());}
    let before:Value=serde_json::from_str(&r.before_json).map_err(|_|"Invalid original state")?;
    let after:Value=serde_json::from_str(&r.after_json).map_err(|_|"Invalid resulting state")?;
    let original_home=before["home_component"].as_str().and_then(|c|c.split_once('/')).map(|(id,_)|id);
    if r.kind=="set_home" {
        let id=original_home.ok_or("Missing original Home")?;
        if scanner::is_protected(id)||before["home_component"].as_str().unwrap_or("").contains("FallbackHome"){return Err("A protected system component cannot be restored as the original launcher.".into());}
        let enabled=before["launchers"][id]["enabled"].as_u64();
        if !matches!(enabled,Some(0|1)){return Err("The recorded original Home was not active.".into());}
        if before["launchers"].as_object().map(|m|m.keys().any(|id|scanner::is_protected(id))).unwrap_or(true){return Err("Protected services cannot be modified by a launcher recovery record.".into());}
    }
    for value in [&before,&after]{
        if value.is_null(){if value==&before&&!["force_stop","trim_cache"].contains(&r.kind.as_str()){return Err("Original recovery state is missing".into());}continue;}
        match r.kind.as_str(){
            "wallpaper_auto"=>{if r.target!="swiftos/auto_wallpaper"||value.as_object().map(|m|m.len())!=Some(1)||!matches!(value["swiftos/auto_wallpaper"].as_str(),Some("0"|"1")){return Err("Invalid local wallpaper recovery setting".into());}},
            "disable"|"enable"|"uninstall"|"install_launcher"=>{
                if !validate_package(&r.target)||scanner::is_protected(&r.target){return Err("Protected or invalid app in recovery export".into());}
                if value["installed"].as_bool().is_none()||value["system"].as_bool().is_none()||value["version_code"].as_u64().is_none(){return Err("Incomplete app recovery state".into());}
                if !value["enabled"].is_null()&&value["enabled"].as_u64().filter(|n|*n<=4).is_none(){return Err("Invalid app enabled state".into());}
                if r.kind=="install_launcher"&&![actions::CUSTOM,actions::ORIGINAL].contains(&r.target.as_str()){return Err("Unknown bundled launcher identity".into());}
            },
            "animations"|"dns"|"patchwall_startup"=>{
                let keys:&[&str]=match r.kind.as_str(){"animations"=>&["global/window_animation_scale","global/transition_animation_scale","global/animator_duration_scale"],"dns"=>&["global/private_dns_mode","global/private_dns_specifier"],_=>&["global/set_patchwall_default"]};
                let object=value.as_object().ok_or("Invalid settings recovery state")?;
                if object.len()!=keys.len()||keys.iter().any(|k|!object.contains_key(*k)){return Err("Unexpected setting in recovery export".into());}
                for setting in object.values(){if !setting.is_null()&&setting.as_str().filter(|s|s.len()<=1024&&!s.contains('\0')).is_none(){return Err("Invalid setting value in recovery export".into());}}
            },
            "set_home"=>{
                let home=value["home_component"].as_str().ok_or("Missing original Home")?;
                let (home_id,activity)=home.split_once('/').ok_or("Invalid Home component")?;
                if !validate_package(home_id)||activity.is_empty()||!activity.bytes().all(|c|c.is_ascii_alphanumeric()||b"._$".contains(&c)){return Err("Invalid Home component".into());}
                let map=value["launchers"].as_object().ok_or("Missing launcher recovery states")?;
                if map.len()>8||!map.contains_key(home_id){return Err("Incomplete Home recovery state".into());}
                for (id,state) in map{
                    if ![actions::CUSTOM,actions::ORIGINAL,actions::GOOGLE,actions::PATCHWALL,actions::PATCHWALL_LEGACY].contains(&id.as_str())&&id!=home_id&&Some(id.as_str())!=original_home{return Err("Unrelated app in Home recovery export".into());}
                    if state["installed"].as_bool().is_none()||state["enabled"].as_u64().filter(|n|*n<=4).is_none(){return Err("Invalid launcher enabled state".into());}
                }
                if !value["patchwall_startup"].is_null()&&value["patchwall_startup"].as_str().filter(|s|s.len()<=128).is_none(){return Err("Invalid startup setting".into());}
            },
            "force_stop"|"trim_cache"=>return Err("Temporary changes must not contain restorable state".into()),
            _=>return Err("Unsupported imported operation".into())
        }
    }
    Ok(())
}

#[cfg(test)]mod tests{
    use super::*;
    #[test]fn imported_home_cannot_disable_recovery_settings(){let mut r=record("set_home",r#"{"home_component":"com.android.tv.settings/.system.FallbackHome","launchers":{"com.android.tv.settings":{"installed":true,"enabled":0}},"patchwall_startup":null}"#);assert!(validate_record(&r).is_err());r.before_json=r#"{"home_component":"com.vendor.home/.Main","launchers":{"com.vendor.home":{"installed":true,"enabled":3}},"patchwall_startup":null}"#.into();assert!(validate_record(&r).is_err());}
    fn record(kind:&str,before:&str)->ChangeRecord{ChangeRecord{id:uuid::Uuid::new_v4().to_string(),session_id:uuid::Uuid::new_v4().to_string(),device_key:"a".repeat(64),serial:"tv".into(),model:"TV".into(),fingerprint:"fw".into(),user_id:0,kind:kind.into(),target:"private_dns".into(),label:"DNS".into(),before_json:before.into(),after_json:"null".into(),status:"prepared".into(),restore_status:"available".into(),error:None,created_at:0,updated_at:0}}
    #[test]fn absent_settings_are_valid_exact_values(){let r=record("dns",r#"{"global/private_dns_mode":null,"global/private_dns_specifier":null}"#);assert!(validate_record(&r).is_ok());}
    #[test]fn unrelated_secure_settings_are_rejected(){let r=record("dns",r#"{"secure/enabled_accessibility_services":"attacker","global/private_dns_specifier":null}"#);assert!(validate_record(&r).is_err());}
    #[test]fn malformed_state_cannot_become_a_restore_command(){let r=record("dns",r#"{"global/private_dns_mode":{"shell":"reboot"},"global/private_dns_specifier":null}"#);assert!(validate_record(&r).is_err());}
    #[test]fn unknown_and_protected_packages_are_rejected(){let mut r=record("disable",r#"{"installed":true,"system":true,"enabled":0,"version_code":1}"#);r.target="com.android.systemui".into();assert!(validate_record(&r).is_err());r.target="com.miui.tv.analytics".into();assert!(validate_record(&r).is_ok());}
}
