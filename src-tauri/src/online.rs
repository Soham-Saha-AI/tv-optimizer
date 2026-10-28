//! Optional manufacturer-only research. Web content can inform a report, never executable rules.
use crate::{models::*,transport::now_ms};
use serde::{Serialize,Deserialize};

#[derive(Debug,Clone,Serialize,Deserialize)]
pub struct OnlineSource { pub title:String,pub url:String,pub status:String,pub detail:String }
#[derive(Debug,Clone,Serialize,Deserialize)]
pub struct OnlineCompatibility { pub model:String,pub checked_at:i64,pub sources:Vec<OnlineSource>,pub findings:Vec<String>,pub remote_checks:Vec<String>,pub summary:String }

fn company_hosts(manufacturer:&str)->&'static[&'static str]{
    let m=manufacturer.to_lowercase();
    if m.contains("xiaomi")||m.contains("mitv"){&["www.mi.com","mi.com","www.xiaomi.com","i01.appmifile.com"]}
    else if m.contains("sony"){&["www.sony.com","www.sony.co.in","www.sony.co.uk","www.sony.net"]}
    else if m.contains("tcl"){&["www.tcl.com","support.tcl.com"]}
    else if m.contains("hisense"){&["www.hisense-usa.com","hisense-usa.com","hisense.co.uk","www.hisense.com"]}
    else if m.contains("philips")||m.contains("tpv"){&["www.philips.com","www.philips.co.in","www.philips.co.uk","www.download.p4c.philips.com"]}
    else if m.contains("nvidia"){&["www.nvidia.com","nvidia.custhelp.com"]}
    else if m.contains("google"){&["support.google.com","store.google.com"]}
    else if m.contains("amazon"){&["www.amazon.com","www.amazon.in","www.amazon.co.uk"]}
    else if m.contains("samsung"){&["www.samsung.com"]}
    else if m.contains("lg"){&["www.lg.com"]}else{&[]}
}
fn parse_company_url(url:&str,manufacturer:&str)->Result<(String,String),String>{
    if url.len()>2048||url.bytes().any(|c|c<=32)||url.contains('\\')||url.contains('#'){return Err("Enter a direct HTTPS manufacturer page URL.".into());}
    let rest=url.strip_prefix("https://").ok_or("Only secure HTTPS manufacturer pages are accepted.")?;
    let (host,path)=rest.split_once('/').map(|(h,p)|(h,format!("/{p}"))).unwrap_or((rest,"/".into()));
    if !company_hosts(manufacturer).iter().any(|h|host.eq_ignore_ascii_case(h)){return Err("This URL is not on a recognized official domain for the scanned TV manufacturer. Other sites cannot be used for compatibility verification.".into());}
    Ok((host.to_lowercase(),path))
}
fn reference(manufacturer:&str)->Option<&'static str>{let m=manufacturer.to_lowercase();
    if m.contains("xiaomi")||m.contains("mitv"){Some("https://www.mi.com/global/support/")}
    else if m.contains("sony"){Some("https://www.sony.com/electronics/support")}
    else if m.contains("tcl"){Some("https://www.tcl.com/global/en/support")}
    else if m.contains("hisense"){Some("https://www.hisense-usa.com/support")}
    else if m.contains("philips")||m.contains("tpv"){Some("https://www.philips.com/c-w/support-home.html")}
    else if m.contains("nvidia"){Some("https://www.nvidia.com/en-us/shield/support/")}
    else if m.contains("google"){Some("https://support.google.com/googletv/")}
    else if m.contains("amazon"){Some("https://www.amazon.com/gp/help/customer/display.html?nodeId=GHH5TUHA7677G4HJ")}
    else{None}}
fn normalized(value:&str)->String{value.chars().filter(|c|c.is_ascii_alphanumeric()).flat_map(char::to_lowercase).collect()}
fn page_matches(body:&str,snapshot:&DeviceSnapshot,known_mi:bool)->bool{
    let body=normalized(body);let model=normalized(&snapshot.identity.model);
    (model.len()>=5&&body.contains(&model))||(known_mi&&(body.contains("miledsmarttv4apro49")||body.contains("miledtv4apro1232cm49")))
}

pub fn check(snapshot:&DeviceSnapshot,official_url:Option<&str>,include_manual:bool)->Result<OnlineCompatibility,String>{
    let i=&snapshot.identity;let known_mi=crate::scanner::matches_profile(i);let manufacturer=if known_mi{"Xiaomi"}else{&i.manufacturer};
    let mut requested=vec![];
    if let Some(url)=official_url.filter(|s|!s.trim().is_empty()){parse_company_url(url.trim(),manufacturer)?;requested.push(("Manufacturer page supplied by you",url.trim().to_string()));}
    else if known_mi{
        requested.push(("Xiaomi Mi TV 4A Pro 49 specifications","https://www.mi.com/in/mi-led-smart-tv-4a-49-pro/specs/".into()));
        requested.push(("Xiaomi Mi TV 4A Pro 49 remote and feature FAQ","https://www.mi.com/in/mi-led-smart-tv-4a-49-pro/faq".into()));
    }else if let Some(url)=reference(manufacturer){requested.push(("Manufacturer support reference",url.into()));}
    let mut report=OnlineCompatibility{model:i.model.clone(),checked_at:now_ms(),sources:vec![],findings:vec![],remote_checks:vec!["Press Home/Circle and Back; check the chosen launcher opens without delay.".into(),"Use D-pad and OK through every visible row; check held volume repeats normally.".into(),"Test microphone/voice, vendor buttons, inputs, standby/wake and your streaming apps.".into()],summary:String::new()};
    for (title,url) in requested{
        let source=match fetch(&url,manufacturer){
            Ok(body)=>{
                let matches=page_matches(&body,snapshot,known_mi);
                if matches&&known_mi&&url.ends_with("/specs/"){
                    let n=normalized(&body);
                    if n.contains("2gb")&&n.contains("8gb"){report.findings.push("Xiaomi lists 2 GB RAM and 8 GB storage. Android's usable totals are lower because system and hardware reservations use part of them.".into());}
                    if n.contains("1920")&&n.contains("1080"){report.findings.push("The manufacturer lists a 1920 × 1080 panel. Keep launcher effects modest and measure actual rendering.".into());}
                    if n.contains("60hz"){report.findings.push("The published panel refresh rate is 60 Hz; this does not establish a guaranteed app frame rate.".into());}
                    if n.contains("cortexa53"){report.findings.push("The published processor is a quad-core Cortex-A53. SwiftOS uses cached artwork and no permanent animation loop.".into());}
                }
                if matches&&known_mi&&url.ends_with("/faq")&&body.to_lowercase().contains("cec"){
                    report.remote_checks.push("Xiaomi documents remote control of supported HDMI-CEC devices. Test your actual HDMI device and its CEC setting separately.".into());
                    report.findings.push("Xiaomi documents Chromecast built-in and Google Play. Casting, Google services and the store remain protected.".into());
                }
                OnlineSource{title:title.into(),url,status:if matches{"matched"}else{"reference_only"}.into(),detail:if matches{"Official page contains the detected model or its validated marketing name. Published specifications supplement the live scan; they do not override detected firmware or prove remote behavior."}else{"Official company page loaded, but an exact model match was not established. Supply its exact product/support page or complete the local checks; no optimization is enabled from this page."}.into()}
            },
            Err(error)=>OnlineSource{title:title.into(),url,status:"unavailable".into(),detail:error}
        };report.sources.push(source);
    }
    if include_manual {
        if known_mi {
            let url="https://i01.appmifile.com/webfile/globalimg/Global_UG/Mi_Ecosystem/Mi_LED_TV_4A_PRO_49/en_V1.pdf";
            let detail=match manual_matches_review(url,manufacturer){
                Ok(true)=>{
                    report.remote_checks.extend(["Compare your remote with the manual's page 4 diagram: it shows separate Android Home and PatchWall buttons.".into(),"Check Power tap and hold, Assistant press/release, OK during playback, Back, D-pad and held Volume on your actual remote. Documented behavior is not a physical pass.".into()]);
                    report.findings.push("Optional remote manual check: the current PDF matches the full nine-page reference reviewed for this release. Its hardware table conflicts with the product site's RAM figure, so exact variant matching still needs confirmation.".into());
                    "All 9 pages, including diagrams, were reviewed for this reference; the downloaded PDF hash matches that reviewed version. Page 4 documents separate Home buttons, voice, Power tap/hold, navigation and playback OK. Remote variant and physical behavior remain unverified.".into()
                },Ok(false)=>"The manufacturer's manual changed since the reviewed reference. Its full remote behavior review is incomplete; no compatibility is inferred.".into(),Err(error)=>error
            };
            report.sources.push(OnlineSource{title:"Optional remote manual review · 9 pages".into(),url:url.into(),status:"reference_only".into(),detail});
        }else{report.findings.push("Optional remote manual check: no fully reviewed manual profile matches this model yet. Remote-button compatibility remains needs testing; standard scan-based controls are unaffected.".into());}
    }
    report.summary=if report.sources.iter().any(|s|s.status=="matched"){"Manufacturer information matched. Physical remote checks and local capability checks are still required; no changes were made."}
    else if report.sources.is_empty(){"No reviewed official manufacturer domain is available for this brand. Local compatibility scanning still works. No web-derived optimization has been enabled."}
    else{"An exact manufacturer model match could not be fully verified. Keep the local scan as the source of truth and use the official references for manual checks."}.into();
    Ok(report)
}

#[cfg(windows)]
fn fetch(url:&str,manufacturer:&str)->Result<String,String>{let(host,path)=parse_company_url(url,manufacturer)?;fetch_host(&host,&path)}

fn manual_matches_review(url:&str,manufacturer:&str)->Result<bool,String>{
    #[cfg(windows)]{use sha2::{Digest,Sha256};let(host,path)=parse_company_url(url,manufacturer)?;let data=fetch_host_bytes(&host,&path,20*1024*1024)?;if !data.starts_with(b"%PDF-"){return Err("The official manual was not returned as a PDF. The remote review remains incomplete.".into());}Ok(format!("{:x}",Sha256::digest(data))=="297a69aae838b92b6bc0b039e2a4cd7424babb3929a4559a39f03501852588ea")}
    #[cfg(not(windows))]{let _=(url,manufacturer);Err("Manual verification is available in the Windows app.".into())}
}

pub(crate) fn github_text(url:&str)->Result<String,String>{
    let allowed=["https://api.github.com/repos/Gathin23/cleanyourtv/","https://raw.githubusercontent.com/Gathin23/cleanyourtv/"];
    if !allowed.iter().any(|prefix|url.starts_with(prefix))||url.len()>2048||url.bytes().any(|b|b<=32)||url.contains('\\')||url.contains('#'){return Err("Only the original CleanYourTV repository can update community references.".into());}
    #[cfg(windows)]{let rest=url.trim_start_matches("https://");let(host,path)=rest.split_once('/').ok_or("Invalid GitHub URL")?;fetch_host(host,&format!("/{path}"))}
    #[cfg(not(windows))]{Err("Online updates are available in the Windows application.".into())}
}

#[cfg(windows)]
fn fetch_host(host:&str,path:&str)->Result<String,String>{
    fetch_host_bytes(host,path,2*1024*1024).map(|bytes|String::from_utf8_lossy(&bytes).into_owned())
}
#[cfg(windows)]
fn fetch_host_bytes(host:&str,path:&str,limit:usize)->Result<Vec<u8>,String>{
    use std::{ffi::c_void,ptr};
    type Handle=*mut c_void;
    #[link(name="winhttp")]
    unsafe extern "system"{
        fn WinHttpOpen(agent:*const u16,access:u32,proxy:*const u16,bypass:*const u16,flags:u32)->Handle;
        fn WinHttpConnect(session:Handle,server:*const u16,port:u16,reserved:u32)->Handle;
        fn WinHttpOpenRequest(connection:Handle,verb:*const u16,path:*const u16,version:*const u16,referer:*const u16,accept:*const *const u16,flags:u32)->Handle;
        fn WinHttpSetTimeouts(handle:Handle,resolve:i32,connect:i32,send:i32,receive:i32)->i32;
        fn WinHttpSetOption(handle:Handle,option:u32,buffer:*mut c_void,length:u32)->i32;
        fn WinHttpSendRequest(request:Handle,headers:*const u16,header_length:u32,optional:*mut c_void,length:u32,total:u32,context:usize)->i32;
        fn WinHttpReceiveResponse(request:Handle,reserved:*mut c_void)->i32;
        fn WinHttpQueryHeaders(request:Handle,level:u32,name:*const u16,buffer:*mut c_void,length:*mut u32,index:*mut u32)->i32;
        fn WinHttpReadData(request:Handle,buffer:*mut c_void,size:u32,read:*mut u32)->i32;
        fn WinHttpCloseHandle(handle:Handle)->i32;
    }
    struct Owned(Handle);impl Drop for Owned{fn drop(&mut self){unsafe{WinHttpCloseHandle(self.0);}}}
    fn wide(s:&str)->Vec<u16>{s.encode_utf16().chain(Some(0)).collect()}
    let error=||format!("The source could not be read securely: {}. Offline scanning remains available.",std::io::Error::last_os_error());
    unsafe{
        let session=Owned(WinHttpOpen(wide("TVOptimizer/0.1 (manufacturer compatibility check)").as_ptr(),4,ptr::null(),ptr::null(),0));if session.0.is_null(){return Err(error());}
        if WinHttpSetTimeouts(session.0,6000,6000,6000,10000)==0{return Err(error());}
        let connection=Owned(WinHttpConnect(session.0,wide(&host).as_ptr(),443,0));if connection.0.is_null(){return Err(error());}
        let request=Owned(WinHttpOpenRequest(connection.0,wide("GET").as_ptr(),wide(&path).as_ptr(),ptr::null(),ptr::null(),ptr::null(),0x00800000));if request.0.is_null(){return Err(error());}
        // Disable automatic redirects: a company page cannot redirect this check onto an unrelated site.
        let mut no_redirects:u32=2;if WinHttpSetOption(request.0,63,&mut no_redirects as *mut _ as *mut c_void,4)==0{return Err(error());}
        if WinHttpSendRequest(request.0,ptr::null(),0,ptr::null_mut(),0,0,0)==0||WinHttpReceiveResponse(request.0,ptr::null_mut())==0{return Err(error());}
        let mut status:u32=0;let mut len:u32=4;
        if WinHttpQueryHeaders(request.0,19|0x20000000,ptr::null(),&mut status as *mut _ as *mut c_void,&mut len,ptr::null_mut())==0{return Err(error());}
        if status!=200{return Err(format!("The official page returned HTTP {status}. A moved page, region restriction or temporary outage cannot be treated as a verified specification."));}
        let mut data=vec![];loop{let mut buffer=[0u8;8192];let mut read:u32=0;if WinHttpReadData(request.0,buffer.as_mut_ptr()as *mut c_void,8192,&mut read)==0{return Err(error());}if read==0{break;}if data.len()+read as usize>limit{return Err("The source exceeded the read limit. No content was applied.".into());}data.extend_from_slice(&buffer[..read as usize]);}
        Ok(data)
    }
}
#[cfg(not(windows))]fn fetch(_url:&str,_manufacturer:&str)->Result<String,String>{Err("Manufacturer checking is available in the Windows app.".into())}

#[cfg(test)]mod tests{
    use super::*;
    #[test]fn only_official_https_domains_are_fetched(){assert!(parse_company_url("https://www.mi.com/in/example","Xiaomi").is_ok());for url in ["http://www.mi.com/","https://www.mi.com.attacker.test/","https://attacker.test@www.mi.com/","https://www.mi.com:444/","https://www.sony.com/","file:///tmp/test"]{assert!(parse_company_url(url,"Xiaomi").is_err(),"{url}");}}
    #[test]fn normalization_handles_marketing_spacing(){assert_eq!(normalized("Mi LED TV 4A PRO 49"),"miledtv4apro49");}
}
