//! Local photo transfer. No downloaded scripts, video playback or resident service.
use crate::{actions::{Engine,same_identity},models::*,scanner,transport::{Adb,quote}};
use serde::{Serialize,Deserialize};
use sha2::{Digest,Sha256};
use std::{collections::BTreeMap,fs,sync::atomic::Ordering};
pub const FOLDER:&str="/sdcard/Auto Wallpaper";
const WEATHER:&[&str]=&["clear","cloudy","fog","drizzle","rain","snow","thunderstorm","unknown","any"];
const TIMES:&[&str]=&["morning","day","evening","night","all"];
#[derive(Serialize)]pub struct Photo{pub name:String,pub weather:String,pub time:String,pub bytes:u64}
#[derive(Serialize)]pub struct Status{pub supported:bool,pub reason:String,pub folder:String,pub enabled:Option<bool>,pub storage_granted:bool,pub images:Vec<Photo>,pub limit:usize}
#[derive(Deserialize)]pub struct Upload{pub name:String,pub weather:String,pub time:String,pub jpeg_base64:String}
#[derive(Serialize)]pub struct Uploaded{pub uploaded:usize,pub folder:String}

fn parse_mode(output:&str)->Result<(bool,bool),String>{
    let mut replies=output.lines().filter_map(|line|line.trim().strip_prefix("Result: Bundle[{").and_then(|s|s.strip_suffix("}]")));
    let reply=replies.next().ok_or("SwiftOS did not return a complete wallpaper status.")?;
    if replies.next().is_some(){return Err("SwiftOS returned ambiguous wallpaper status.".into())}
    let mut fields=BTreeMap::new();
    for field in reply.split(','){
        let (key,value)=field.trim().split_once('=').ok_or("Invalid wallpaper status field.")?;
        if fields.insert(key.trim(),value.trim()).is_some(){return Err("Duplicate wallpaper status field.".into())}
    }
    if fields.get("protocol")!=Some(&"1"){return Err("Install the compatible local-wallpaper SwiftOS v3 build before using this control.".into())}
    let boolean=|key:&str|match fields.get(key){Some(&"true")=>Ok(true),Some(&"false")=>Ok(false),_=>Err("SwiftOS did not confirm its wallpaper setting and photo access.".to_string())};
    Ok((boolean("enabled")?,boolean("storage_granted")?))
}
pub fn read_mode(adb:&Adb,serial:&str)->Result<(bool,bool),String>{
    parse_mode(&adb.shell(serial,"content call --user 0 --uri content://com.gathin.tvhome.theme --method local_wallpaper_status")?)
}
pub fn set_mode(adb:&Adb,serial:&str,on:bool)->Result<(),String>{
    let (_,permission)=read_mode(adb,serial)?;
    if on&&!permission{return Err("Allow photo access in SwiftOS on the TV first. Open launcher Settings, Auto wallpaper, then accept the photo permission prompt.".into())}
    let out=adb.shell(serial,&format!("content call --user 0 --uri content://com.gathin.tvhome.theme --method set_local_wallpaper --arg {}",if on{"on"}else{"off"}))?;
    if parse_mode(&out)?.0!=on{return Err("SwiftOS did not confirm this wallpaper choice.".into())}
    if read_mode(adb,serial)?.0!=on{return Err("The wallpaper setting changed before verification. Refresh and review again.".into())}
    Ok(())
}
fn compatible(s:&DeviceSnapshot)->bool{
    s.identity.user_id==0&&scanner::matches_profile(&s.identity)
        &&crate::recommendations::capability(s,"Stable TV identity")
        &&s.packages.iter().any(|p|p.id==scanner::LAUNCHER&&p.installed&&p.version_code>=24)
}
fn guard_identity(e:&Engine,serial:&str,expected:&DeviceIdentity)->Result<(),String>{
    let (current,hardware)=scanner::read_identity(&e.adb,serial)?;
    if hardware.starts_with("connection:"){return Err("The TV's stable identity could not be verified. Photo transfer stopped.".into())}
    same_identity(expected,&current)
}
fn photos(adb:&Adb,serial:&str)->Result<Vec<Photo>,String>{
    // Bound TV work and desktop output even if the shared folder was populated manually.
    let out=adb.shell(serial,&format!("if [ -d {folder} ]; then find {folder} -maxdepth 3 -type f -exec stat -c '%s|%n' {{}} \\; | head -n 1025; fi",folder=quote(FOLDER)))?;
    if out.lines().count()>=1025{return Err("The Auto Wallpaper folder has too many files to inspect safely. Keep the collection below 256 photos and refresh.".into())}
    let mut images=vec![];
    for line in out.lines(){
        let Some((size,path))=line.split_once('|')else{continue};
        let Some(relative)=path.strip_prefix(&format!("{FOLDER}/"))else{continue};
        let parts:Vec<_>=relative.split('/').collect();
        let (weather,time,name)=match parts.as_slice(){
            [name]=>("any","all",*name),
            [w,name]if WEATHER.contains(w)=>(*w,"all",*name),
            [w,t,name]if WEATHER.contains(w)&&TIMES.contains(t)=>(*w,*t,*name),
            _=>continue
        };
        if ![".jpg",".jpeg",".png",".webp"].iter().any(|suffix|name.to_lowercase().ends_with(suffix)){continue}
        images.push(Photo{name:name.into(),weather:weather.into(),time:time.into(),bytes:size.parse().unwrap_or(0)});
    }
    Ok(images)
}
pub fn status(e:&Engine,serial:&str)->Result<Status,String>{
    let s=scanner::scan(&e.adb,serial)?;
    let mut result=Status{supported:false,reason:"Local wallpaper controls are tested only for the primary TV user on Mi TV 4A Pro with the local-wallpaper SwiftOS v3 update. Build 22 and the original tested build 23 remain available separately.".into(),folder:FOLDER.into(),enabled:None,storage_granted:false,images:vec![],limit:256};
    if !compatible(&s){return Ok(result)}
    let (enabled,permission)=read_mode(&e.adb,serial)?;
    result.supported=true;result.enabled=Some(enabled);result.storage_granted=permission;
    result.reason="Images stay on the TV. Press Home after changing photo files. Weather uses SwiftOS's existing Kolkata forecast; missing or stale weather uses Unknown or Any-weather photos.".into();
    result.images=photos(&e.adb,serial)?;
    guard_identity(e,serial,&s.identity)?;
    Ok(result)
}
pub fn upload(e:&Engine,serial:&str,key:&str,images:Vec<Upload>,progress:impl Fn(OperationProgress))->Result<Uploaded,String>{
    if images.is_empty()||images.len()>30{return Err("Choose between 1 and 30 photos at a time.".into())}
    let s=scanner::scan(&e.adb,serial)?;
    if s.identity.device_key!=key||!compatible(&s){return Err("The selected TV or SwiftOS version changed. Scan and check wallpaper compatibility again.".into())}
    read_mode(&e.adb,serial)?;
    if photos(&e.adb,serial)?.len()+images.len()>256{return Err("Keep this collection below 256 images. Move older photos using the TV file manager before transferring more.".into())}
    // Validate the complete batch before the first TV write.
    let mut prepared=Vec::new();let mut total_bytes=0usize;
    for item in images{
        if !WEATHER.contains(&item.weather.as_str())||!TIMES.contains(&item.time.as_str()){return Err("Choose a supported weather and time folder.".into())}
        let data=decode_base64(&item.jpeg_base64)?;jpeg_size(&data)?;
        total_bytes+=data.len();if total_bytes>20*1024*1024{return Err("Transfer at most 20 MB of prepared photos per batch.".into())}
        prepared.push((item,data));
    }
    let directory=e.root.join("data/wallpaper-transfers");
    fs::create_dir_all(&directory).map_err(|x|x.to_string())?;
    e.cancelled.store(false,Ordering::SeqCst);
    let total=prepared.len();let mut uploaded=0;
    for (item,data) in prepared{
        if e.cancelled.load(Ordering::SeqCst){break}
        let token=uuid::Uuid::new_v4().to_string();
        let slug:String=item.name.chars().filter(|c|c.is_ascii_alphanumeric()||*c=='-'||*c=='_').take(32).collect();
        let name=format!("{}-{token}.jpg",if slug.is_empty(){"photo"}else{&slug});
        let local=directory.join(format!("{token}.jpg"));
        let temp=format!("/data/local/tmp/tvoptimizer-wallpaper-{token}.jpg");
        let destination=format!("{FOLDER}/{}/{}",item.weather,item.time);
        let final_path=format!("{destination}/{name}");let pending=format!("{final_path}.pending");
        let expected=format!("{:x}",Sha256::digest(&data));
        let transfer:Result<(),String>=(||{
            fs::write(&local,&data).map_err(|x|x.to_string())?;
            guard_identity(e,serial,&s.identity)?;
            e.adb.push(serial,&local,&temp)?;
            let digest=e.adb.shell(serial,&format!("sha256sum {}",quote(&temp)))?;
            if digest.split_whitespace().next()!=Some(expected.as_str()){return Err("Photo transfer checksum did not match.".into())}
            guard_identity(e,serial,&s.identity)?;
            if e.cancelled.load(Ordering::SeqCst){return Err("Transfer was stopped before saving this photo.".into())}
            let out=e.adb.shell(serial,&format!(
                "mkdir -p {folder} && cp {temp} {pending} && mv {pending} {final_path} && touch {revision} && printf tvopt_photo_saved",
                folder=quote(&destination),temp=quote(&temp),pending=quote(&pending),final_path=quote(&final_path),revision=quote(&format!("{FOLDER}/.revision"))))?;
            if !out.ends_with("tvopt_photo_saved"){return Err("The TV did not confirm saving this photo. Check its free storage.".into())}
            Ok(())
        })();
        let _=fs::remove_file(&local);
        // Clean only this transfer's unique temporary files, and only on the same TV.
        if guard_identity(e,serial,&s.identity).is_ok(){
            let _=e.adb.shell(serial,&format!("rm -f {} {}",quote(&temp),quote(&pending)));
        }
        if let Err(error)=transfer{
            return Err(format!("{uploaded} of {total} photos were confirmed saved. {error} The last photo may need reconciliation; refresh the collection before choosing missing photos. No photos were automatically deleted or retried."));
        }
        uploaded+=1;
        progress(OperationProgress{phase:"uploading".into(),message:format!("Saved photo {uploaded} of {total} on your TV"),completed:uploaded,total,change_id:None});
    }
    Ok(Uploaded{uploaded,folder:FOLDER.into()})
}
fn decode_base64(text:&str)->Result<Vec<u8>,String>{
    if text.len()>2_800_000||text.len()%4!=0{return Err("A prepared photo is too large or invalid.".into())}
    let mut result=Vec::with_capacity(text.len()/4*3);let bytes=text.as_bytes();
    for(i,chunk)in bytes.chunks_exact(4).enumerate(){
        let mut bits=0u32;let mut pad=0;
        for (j,&c)in chunk.iter().enumerate(){
            let v=match c{b'A'..=b'Z'=>c-b'A',b'a'..=b'z'=>c-b'a'+26,b'0'..=b'9'=>c-b'0'+52,b'+'=>62,b'/'=>63,b'=' if j>=2&&i==bytes.len()/4-1=>{pad+=1;0},_=>return Err("Invalid photo encoding".into())};
            if pad>0&&c!=b'=' {return Err("Invalid photo padding".into())}
            bits=(bits<<6)|v as u32;
        }
        result.push((bits>>16)as u8);if pad<2{result.push((bits>>8)as u8)}if pad<1{result.push(bits as u8)}
    }
    if result.len()>2*1024*1024{return Err("Prepare photos below 2 MB.".into())}Ok(result)
}
fn jpeg_size(data:&[u8])->Result<(u16,u16),String>{
    if !data.starts_with(&[255,216])||!data.ends_with(&[255,217]){return Err("Transfer a prepared JPEG photo.".into())}
    let mut p=2;
    while p+4<data.len(){
        if data[p]!=255{return Err("Invalid JPEG segment".into())}
        let marker=data[p+1];p+=2;if marker==255{p-=1;continue}
        let length=u16::from_be_bytes([data[p],data[p+1]])as usize;if length<2||p+length>data.len(){break}
        if matches!(marker,0xc0|0xc1|0xc2)&&length>=8{
            let h=u16::from_be_bytes([data[p+3],data[p+4]]);let w=u16::from_be_bytes([data[p+5],data[p+6]]);
            if w==0||h==0||w>1920||h>1080{return Err("Resize photos to at most 1920 × 1080 before transferring.".into())}return Ok((w,h))
        }
        p+=length;
    }
    Err("Photo dimensions could not be verified.".into())
}
#[cfg(test)]mod tests{
    use super::*;
    #[test]fn protocol_requires_exact_complete_unambiguous_state(){
        assert_eq!(parse_mode("Result: Bundle[{enabled=false, protocol=1, storage_granted=true}]").unwrap(),(false,true));
        for bad in ["Result: Bundle[{enabled=false, protocol=10, storage_granted=true}]","Result: Bundle[{protocol=1, storage_granted=true}]","Result: Bundle[{enabled=true, enabled=false, protocol=1, storage_granted=true}]","Result: Bundle[{enabled=no, protocol=1, storage_granted=true}]"]{assert!(parse_mode(bad).is_err())}
    }
    #[test]fn photo_transport_rejects_unbounded_or_malformed_payloads(){
        assert_eq!(decode_base64("/9j/2Q==").unwrap(),vec![255,216,255,217]);assert!(decode_base64("Z=AA").is_err());assert!(jpeg_size(&[255,216,255,217]).is_err());
        let mut jpeg=vec![255,216,255,192,0,17,8,4,56,7,128,3,1,17,0,2,17,0,3,17,0,255,217];
        assert_eq!(jpeg_size(&jpeg).unwrap(),(1920,1080));jpeg[10]=129;assert!(jpeg_size(&jpeg).is_err());
    }
}

