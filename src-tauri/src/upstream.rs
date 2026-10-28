//! Public compatibility references only. Downloaded prose never becomes executable rules.
use serde::{Deserialize,Serialize};
use std::{fs,path::Path};
use crate::{online::github_text,transport::now_ms};
const REPOSITORY:&str="https://github.com/Gathin23/cleanyourtv";
#[derive(Clone,Debug,Serialize,Deserialize)]
struct Catalog {updated_at:i64,revision:String,release_tag:String,release_url:String,apk_available:bool,models:Vec<String>}
#[derive(Clone,Debug,Serialize,Deserialize)]
pub struct CommunityStatus {pub updated_at:Option<i64>,pub revision:String,pub source_url:String,pub release_tag:String,pub release_url:String,pub apk_available:bool,pub models_count:usize,pub matches:Vec<String>,pub message:String,pub update_error:Option<String>}
fn normalized(s:&str)->String{s.chars().filter(|c|c.is_ascii_alphanumeric()).flat_map(char::to_lowercase).collect()}
fn parse_models(readme:&str)->Vec<String>{
    let mut rows=vec![];
    for line in readme.lines().filter(|s|s.trim_start().starts_with('|')){
        let cells:Vec<_>=line.trim().trim_matches('|').split('|').map(str::trim).collect();
        if cells.len()<2||cells[0].len()<5||cells[0].len()>512{continue;}
        if !["android","google tv","fire os"].iter().any(|os|cells[1].to_lowercase().contains(os)){continue;}
        let item=format!("{} · {}",cells[0],cells[1]);if !rows.contains(&item){rows.push(item);}
        if rows.len()>=2000{break;}
    }rows
}
fn load(root:&Path)->Option<Catalog>{let path=root.join("data/community/catalog.json");let data=fs::read(path).ok()?;if data.len()>2*1024*1024{return None;}let c:Catalog=serde_json::from_slice(&data).ok()?;if c.revision.len()!=40||!c.revision.bytes().all(|b|b.is_ascii_hexdigit())||c.models.len()>2000{return None;}Some(c)}
pub fn status(root:&Path,model:&str,refresh:bool)->Result<CommunityStatus,String>{
    static UPDATE_LOCK:std::sync::Mutex<()>=std::sync::Mutex::new(());let _guard=UPDATE_LOCK.lock().map_err(|_|"Community cache is busy. Try again later.")?;
    if model.len()>256{return Err("Invalid model reference".into());}
    let mut cached=load(root);let mut error=None;
    if refresh{
        match download(){Ok(c)=>{let dir=root.join("data/community");fs::create_dir_all(&dir).map_err(|e|e.to_string())?;let temp=dir.join("catalog.pending.json");fs::write(&temp,serde_json::to_vec(&c).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;fs::rename(temp,dir.join("catalog.json")).map_err(|e|e.to_string())?;cached=Some(c);},Err(e)=>error=Some(e)}
    }
    let Some(c)=cached else{return Ok(CommunityStatus{updated_at:None,revision:String::new(),source_url:REPOSITORY.into(),release_tag:String::new(),release_url:format!("{REPOSITORY}/releases/latest"),apk_available:false,models_count:0,matches:vec![],message:"Bundled offline rules remain available. Connect to the Internet to refresh community references; no TV changes are made.".into(),update_error:error});};
    let model=normalized(model);let matches=if model.len()>=4{c.models.iter().filter(|s|normalized(s).contains(&model)).take(12).cloned().collect()}else{vec![]};
    Ok(CommunityStatus{updated_at:Some(c.updated_at),revision:c.revision,source_url:REPOSITORY.into(),release_tag:c.release_tag,release_url:c.release_url,apk_available:c.apk_available,models_count:c.models.len(),matches,message:if c.apk_available{"The publisher lists an APK asset. This build does not install an unreviewed new binary: package, signing identity, Android requirements and conflict checks must pass before a release can be offered."}else{"The latest official release provides source code. Use Get latest CleanYourTV in Launchers to download, build and verify it in the background. SwiftOS is included offline."}.into(),update_error:error})
}
fn download()->Result<Catalog,String>{
    let commit:serde_json::Value=serde_json::from_str(&github_text("https://api.github.com/repos/Gathin23/cleanyourtv/commits/main")?).map_err(|_|"Invalid GitHub revision response")?;
    let sha=commit["sha"].as_str().filter(|s|s.len()==40&&s.bytes().all(|b|b.is_ascii_hexdigit())).ok_or("GitHub did not supply a valid revision")?;
    let readme=github_text(&format!("https://raw.githubusercontent.com/Gathin23/cleanyourtv/{sha}/README.md"))?;
    let models=parse_models(&readme);if models.is_empty(){return Err("The updated community index could not be interpreted. The previous database was kept.".into());}
    let release:serde_json::Value=serde_json::from_str(&github_text("https://api.github.com/repos/Gathin23/cleanyourtv/releases/latest")?).map_err(|_|"Invalid GitHub release response")?;
    if release["draft"]==true||release["prerelease"]==true{return Err("The publisher did not return a stable release.".into());}
    let tag=release["tag_name"].as_str().filter(|s|!s.is_empty()&&s.len()<128).ok_or("Missing release version")?;
    let url=release["html_url"].as_str().filter(|s|s.starts_with(&format!("{REPOSITORY}/releases/tag/"))).ok_or("The release belongs to an unexpected repository")?;
    let apk=release["assets"].as_array().map(|a|a.iter().any(|a|a["name"].as_str().map(|s|s.to_lowercase().ends_with(".apk")).unwrap_or(false)&&a["browser_download_url"].as_str().map(|s|s.starts_with(&format!("{REPOSITORY}/releases/download/"))).unwrap_or(false))).unwrap_or(false);
    Ok(Catalog{updated_at:now_ms(),revision:sha.into(),release_tag:tag.into(),release_url:url.into(),apk_available:apk,models})
}
#[cfg(test)]mod tests{use super::*;#[test]fn prose_is_reference_data_only(){let rows=parse_models("| TV | OS |\n| Mi TV (MiTV-AXSO1) | Android TV 9 |\n| Sony TV | Google TV 12 |\n| sh evil.sh | execute me |\n");assert_eq!(rows.len(),2);assert!(normalized(&rows[0]).contains(&normalized("MiTV-AXSO1")));}#[test]fn disallows_lookalike_repository_urls(){assert!(github_text("https://api.github.com.evil.test/repos/Gathin23/cleanyourtv/main").is_err());assert!(github_text("https://raw.githubusercontent.com/other/repo/main/install.sh").is_err());}}
