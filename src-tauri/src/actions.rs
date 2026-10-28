//! Approved actions, durable before-images, read-back verification, and targeted recovery.
use crate::{journal::Journal, models::*, scanner, transport::{Adb,now_ms,quote,validate_package}};
use crate::launcher_variant::{hash_is_approved,SWIFTOS_APK_NAME,SWIFTOS_HASH};
use serde_json::{json,Value};
use sha2::{Digest,Sha256};
use std::{collections::BTreeSet,fs,io::Write,path::PathBuf,sync::{Arc,atomic::{AtomicBool,Ordering}},thread,time::Duration};

pub const CUSTOM:&str="com.gathin.tvhome";
pub const ORIGINAL:&str="com.gathin.tvhome.original";
pub const GOOGLE:&str="com.google.android.tvlauncher";
pub const PATCHWALL:&str="com.mitv.tvhome.atv";
pub const PATCHWALL_LEGACY:&str="com.mitv.tvhome";

#[derive(Clone)]
pub struct Engine { pub root:PathBuf,pub adb:Adb,pub journal:Journal,pub cancelled:Arc<AtomicBool> }

pub fn dns_hostname(value:&str)->Option<&'static str> {
    match value { "adguard"|"dns.adguard-dns.com"=>Some("dns.adguard-dns.com"),"pro"|"x-hagezi-pro.freedns.controld.com"=>Some("x-hagezi-pro.freedns.controld.com"),"proplus"|"x-hagezi-proplus.freedns.controld.com"=>Some("x-hagezi-proplus.freedns.controld.com"),"ultimate"|"x-hagezi-ultimate.freedns.controld.com"=>Some("x-hagezi-ultimate.freedns.controld.com"),_=>None }
}
fn is_launcher(id:&str)->bool { [CUSTOM,ORIGINAL,GOOGLE,PATCHWALL,PATCHWALL_LEGACY].contains(&id) }
fn resource_key(a:&ActionRequest)->String { if ["disable","enable","uninstall","force_stop"].contains(&a.kind.as_str()){format!("package:{}",a.target)}else{a.kind.clone()} }
fn valid_component(component:&str)->bool { component.split_once('/').map(|(p,c)|validate_package(p)&&!c.is_empty()&&c.bytes().all(|b|b.is_ascii_alphanumeric()||b"._$".contains(&b))).unwrap_or(false) }
fn package<'a>(s:&'a DeviceSnapshot,id:&str)->Result<&'a PackageInfo,String> { s.packages.iter().find(|p|p.id==id).ok_or_else(||format!("The scan could not verify {id}. Scan again before changing it.")) }
fn active(p:&PackageInfo)->bool { p.installed&&matches!(p.enabled,Some(0|1)) }
fn restore_package_allowed(p:&PackageInfo)->bool{!p.protected&&!scanner::is_protected(&p.id)&&(!p.system||scanner::allowed_optional(&p.id)||is_launcher(&p.id))}
fn uninstall_warning(p:&PackageInfo)->String {
    let state=if p.installed&&matches!(p.enabled,Some(2..=4)){" is already disabled. Uninstalling may not improve speed or RAM further."}else{" will be uninstalled for this TV user."};
    let recovery=if p.system{"The factory APK stays on the TV, so storage savings can be small. Updates, private data and sign-ins may not be recoverable."}else{"A verified APK backup does not guarantee recovery of private data, downloads or sign-ins. Storage savings are not guaranteed."};
    format!("{}{state} {recovery}",p.label)
}
fn require_setting(s:&DeviceSnapshot,key:&str)->Result<(),String> {
    if !s.settings.contains_key(key) { return Err(format!("Could not read {key}; its original value cannot be safely restored.")); } Ok(())
}
pub(crate) fn same_identity(saved:&DeviceIdentity,current:&DeviceIdentity)->Result<(),String> {
    if saved.device_key!=current.device_key||saved.user_id!=current.user_id {return Err("The connected TV or Android user changed. Nothing was applied.".into());}
    if saved.fingerprint!=current.fingerprint {return Err("The TV firmware changed. Scan and review compatibility again.".into());} Ok(())
}

impl Engine {
    pub fn baseline_status(&self,s:&DeviceSnapshot)->Result<Option<BaselineStatus>,String>{
        let Some(original)=self.journal.baseline(&s.identity.device_key)? else{return Ok(None)};
        let records=self.journal.baseline_changes(&s.identity.device_key)?;
        Ok(Some(BaselineStatus{saved_at:original.scanned_at,model:original.identity.model.clone(),pre_existing:scanner::existing_optimizations(&original),restorable_changes:records.iter().filter(|r|restorable(r)).count(),non_reversible_changes:records.iter().filter(|r|r.restore_status=="not_available"&&r.status!="failed").count(),review_required:records.iter().filter(|r|r.restore_status=="needs_review"||r.status=="prepared").count()}))
    }
    pub fn preview_restore_all(&self,serial:&str)->Result<RestoreAllPreview,String>{
        let current=scanner::scan(&self.adb,serial)?;
        let original=self.journal.baseline(&current.identity.device_key)?.ok_or("No starting restore point is saved for this TV. Scan it before making changes.")?;
        same_identity(&original.identity,&current.identity)?;
        self.reconcile(&current)?;
        restore_all_plan(&original,&current,&self.journal.baseline_changes(&current.identity.device_key)?)
    }
    pub fn restore_all(&self,serial:&str,plan:RestoreAllPreview,progress:impl Fn(OperationProgress))->Result<Vec<ChangeRecord>,String>{
        if now_ms().saturating_sub(plan.created_at)>600_000||plan.created_at>now_ms()+30_000{return Err("This restore review expired. Review Restore all again.".into());}
        let fresh=self.preview_restore_all(serial)?;
        if fresh.device_key!=plan.device_key||fresh.fingerprint!=plan.fingerprint||fresh.baseline_at!=plan.baseline_at||fresh.change_ids!=plan.change_ids||fresh.preconditions!=plan.preconditions||fresh.skipped!=plan.skipped{return Err("The TV or its recovery history changed after review. Review Restore all again before continuing.".into());}
        self.cancelled.store(false,Ordering::SeqCst);let total=plan.change_ids.len();let mut restored=Vec::new();
        for (i,id) in plan.change_ids.iter().enumerate(){
            if self.cancelled.load(Ordering::SeqCst){break;}
            progress(OperationProgress{phase:"restoring".into(),message:format!("Restoring starting state: {}",plan.labels.get(i).map(String::as_str).unwrap_or("saved change")),completed:i,total,change_id:Some(id.clone())});
            restored.push(self.restore(serial,id)?);
        }
        progress(OperationProgress{phase:"restoring".into(),message:if restored.len()==total{"Starting-state restoration finished. Review any listed limits."}else{"Restoration stopped safely between changes."}.into(),completed:restored.len(),total,change_id:None});
        Ok(restored)
    }
    /// Reconnect reconciliation reads the TV only. It never retries an interrupted command.
    pub fn reconcile(&self,s:&DeviceSnapshot)->Result<(),String>{
        for mut r in self.journal.list(Some(&s.identity.device_key))? {
            if !matches!(r.status.as_str(),"prepared"|"uncertain"|"awaiting_confirmation")||r.restore_status=="restored"||r.restore_status=="not_available"||guard_record(&r,s).is_err(){continue;}
            if r.status=="awaiting_confirmation"&&now_ms().saturating_sub(r.created_at)<160_000{continue;}
            let a=ActionRequest{kind:r.kind.clone(),target:r.target.clone(),value:String::new()};
            let Ok(actual)=capture(s,&a) else{continue;};
            let Ok(before)=serde_json::from_str::<Value>(&r.before_json) else{continue;};
            let Ok(after)=serde_json::from_str::<Value>(&r.after_json) else{continue;};
            match interrupted_state(&r.kind,&before,&after,&actual){
                "original"=>{r.status="failed".into();r.restore_status="restored".into();r.error=Some("Checked after reconnect: the recorded original state is present. No command was retried.".into());},
                "recorded"=>{r.restore_status="available".into();if r.kind=="set_home"{r.error=Some("Home confirmation expired and the recorded Home is still active. Review Restore to return to the previous configuration.".into());}else{r.error=Some("The recorded result is present after reconnect. Review the change before restoring; no command was retried.".into());}r.status="uncertain".into();},
                _=>{r.status="uncertain".into();r.restore_status="needs_review".into();r.error=Some("This interrupted change does not match a verified result. A later manual change or partial operation may be present. Automatic retry and restore are blocked to preserve the current state; export this recovery record for review.".into());}
            }
            self.save(&mut r)?;
        }Ok(())
    }
    pub fn preview(&self,serial:&str,actions:Vec<ActionRequest>)->Result<PlanPreview,String> {
        if actions.is_empty()||actions.len()>100 {return Err("Choose between one and 100 individual changes.".into());}
        let s=scanner::scan(&self.adb,serial)?; let mut unique=BTreeSet::new();let mut warnings=vec![];let mut preconditions=std::collections::BTreeMap::new();
        for a in &actions {
            self.validate(&s,a)?;
            let group=match a.kind.as_str(){"disable"|"enable"|"uninstall"|"force_stop"=>format!("package:{}",a.target),_=>a.kind.clone()};
            if !unique.insert(group) {return Err("Choose only one change for each app or setting at a time.".into());}
            preconditions.insert(resource_key(a),capture(&s,a)?.to_string());
            if a.kind=="uninstall" {warnings.push(uninstall_warning(package(&s,&a.target)?));}
            if a.kind=="dns"&&matches!(a.value.as_str(),"ultimate"|"proplus"|"x-hagezi-proplus.freedns.controld.com"|"x-hagezi-ultimate.freedns.controld.com") {warnings.push("Stronger DNS filters can stop sign-in, recommendations or playback. Test your apps; Restore returns to your previous provider.".into());}
            if a.kind=="trim_cache" {warnings.push("Deleted cache has no Undo. Apps will rebuild it and may open more slowly once.".into());}
            if a.kind=="disable"&&a.target==PATCHWALL {warnings.push("Disabling PatchWall can stop the Mi button opening it. Remote remapping is not enabled.".into());}
        }
        if actions.iter().any(|a|a.kind=="set_home")&&actions.len()>1 {return Err("Switch and confirm Home as a separate step. Then choose which old launchers to disable.".into());}
        warnings.sort();warnings.dedup();
        Ok(PlanPreview{device_key:s.identity.device_key,fingerprint:s.identity.fingerprint,actions,warnings,created_at:now_ms(),preconditions})
    }

    pub fn validate(&self,s:&DeviceSnapshot,a:&ActionRequest)->Result<(),String> {
        if !crate::recommendations::capability(s,"Android TV platform"){return Err("The scan could not verify an Android TV platform. Inventory works, but TV optimizations are unavailable.".into());}
        if s.inspections.iter().any(|i|i.name=="Stable TV identity"&&i.status=="could_not_check"){return Err("This TV did not expose a stable identity. Inventory works, but changes are blocked to keep recovery tied to the correct TV.".into());}
        match a.kind.as_str() {
            "disable"|"enable"|"uninstall"|"force_stop"=>{
                if !validate_package(&a.target){return Err("Invalid app identifier".into());}
                let p=package(s,&a.target)?;
                if !crate::recommendations::capability(s,"Package disable"){return Err("The initial scan could not verify per-user app control on this TV.".into());}
                if a.kind=="uninstall"&&!crate::recommendations::capability(s,"Package uninstall"){return Err("The initial scan could not verify per-user uninstall on this TV.".into());}
                if p.protected||scanner::is_protected(&a.target) {return Err("This app is protected because the TV needs it to function or recover.".into());}
                if p.enabled.is_none(){return Err("The app's original enabled state could not be read.".into());}
                if !p.installed&&!(a.kind=="enable"&&p.system&&scanner::allowed_optional(&a.target)){return Err("This app is not currently installed for the selected TV user.".into());}
                if a.kind=="force_stop" {if p.system||p.is_launcher{return Err("Only ordinary user apps can be closed temporarily.".into());}}
                else if p.system&&!scanner::allowed_optional(&a.target)&&!is_launcher(&a.target) {return Err("No reviewed removal rule exists for this system app. Unknown dependencies are not guessed.".into());}
                if a.kind=="uninstall"&&p.system&&s.profile.is_none(){return Err("System-app removal is only enabled on a validated firmware profile. Disable remains available for recognized optional apps.".into());}
                if is_launcher(&a.target)&&a.kind=="uninstall" {return Err("Keep launcher apps installed for recovery. Disable them instead.".into());}
                if (p.is_launcher||is_launcher(&a.target))&&a.kind=="disable" {
                    if s.home_component.as_deref().map(|h|h.starts_with(&format!("{}/",a.target))).unwrap_or(false){return Err("Choose and confirm a different Home screen before disabling this one.".into());}
                    if !self.confirmed_home(s)?{return Err("Use Make this my Home screen, press Home on your remote and confirm it works before disabling an old launcher.".into());}
                }
            },
            "animations"=>{if !crate::recommendations::capability(s,"Animations capability")||!["0","0.5","1"].contains(&a.value.as_str()){return Err("The scan could not verify this animation control.".into());}for k in animation_keys(){require_setting(s,k)?;}},
            "dns"=>{if !crate::recommendations::capability(s,"Private DNS")||s.identity.api<28||dns_hostname(&a.value).is_none(){return Err("This DNS choice requires a compatible Android 9+ TV with working Private DNS controls.".into());}for k in dns_keys(){require_setting(s,k)?;}},
            "patchwall_startup"=>{if s.profile.is_none()||a.value!="0"{return Err("This startup override is only verified on the matching Mi TV firmware.".into());}require_setting(s,"global/set_patchwall_default")?;if !self.confirmed_home(s)?{return Err("Confirm your replacement Home first.".into());}},
            "wallpaper_auto"=>{if s.identity.user_id!=0||!scanner::matches_profile(&s.identity)||a.target!="swiftos/auto_wallpaper"||!matches!(a.value.as_str(),"0"|"1"){return Err("This local wallpaper control is supported only on the validated Mi TV profile.".into());}require_setting(s,"swiftos/auto_wallpaper")?;if a.value=="1"&&!crate::recommendations::capability(s,"Local wallpaper photo access"){return Err("Allow SwiftOS photo access on the TV, then scan again before enabling local wallpaper.".into());}},
            "install_launcher"=>{if a.target==CUSTOM{if !scanner::matches_profile(&s.identity){return Err("SwiftOS is currently validated only for the Mi TV 4A Pro firmware profile.".into());}let maximum=if a.value=="build22"{22}else{crate::launcher_variant::SWIFTOS_VERSION_CODE};if s.packages.iter().any(|p|p.id==CUSTOM&&p.installed&&p.version_code>maximum){return Err("A newer SwiftOS is installed. Automatic downgrades are blocked to preserve your settings; the older APK stays in the package.".into());}}if s.identity.api<26{return Err("These launchers require Android TV 8 or later.".into());}if ![CUSTOM,ORIGINAL].contains(&a.target.as_str()){return Err("Only verified launchers can be installed.".into());}if a.target==ORIGINAL{let(_,ready)=crate::builder::original_path(&self.root)?;if s.identity.api<ready.minimum_sdk{return Err("This release needs a newer Android version than your TV provides.".into());}}self.launcher_path(&a.target,&a.value)?;},
            "set_home"=>{
                if !crate::recommendations::capability(s,"Home switch")||!crate::recommendations::capability(s,"Detached recovery"){return Err("The initial scan could not verify Home switching and detached recovery on this TV.".into());}
                require_setting(s,"global/set_patchwall_default")?;
                if !valid_component(&a.target){return Err("Invalid Home activity".into());}
                let id=a.target.split('/').next().unwrap();if ![CUSTOM,ORIGINAL,GOOGLE,PATCHWALL].contains(&id){return Err("This Home screen is not supported by the recovery flow.".into());}
                if !active(package(s,id)?){return Err("Install or enable this launcher first.".into());}
                if ![CUSTOM,ORIGINAL].contains(&id)&&!s.inspections.iter().filter(|i|i.name=="Home activities"&&i.status=="detected").flat_map(|i|i.detail.lines()).any(|c|normalize_component(c)==normalize_component(&a.target)){return Err("This app did not advertise the requested Android Home activity. Its TV content screen or OEM startup switch is a separate feature.".into());}
                let old=s.home_component.as_deref().ok_or("The current Home could not be identified; takeover is blocked.")?;
                if !valid_component(old)||old.contains("FallbackHome"){return Err("A usable previous Home is required for recovery.".into());}
                let old_id=old.split('/').next().unwrap();let old_pkg=package(s,old_id)?;
                if !active(old_pkg){return Err("The previous Home's exact active app state could not be verified; takeover is blocked.".into());}
                if !["","replace_google"].contains(&a.value.as_str()){return Err("Invalid Home replacement option".into());}
                if a.value=="replace_google"&&(s.profile.is_none()||id==GOOGLE){return Err("Google replacement is only supported for the verified Mi TV profile.".into());}
                let pending=self.journal.list(Some(&s.identity.device_key))?.iter().any(|r|r.status=="awaiting_confirmation"&&r.restore_status!="restored"&&now_ms()-r.created_at<180_000);
                if pending{return Err("Finish or restore the pending Home confirmation first.".into());}
            },
            "trim_cache"=>{if s.profile.is_none()||a.value!="256M"{return Err("Cache cleanup is only verified on the matching Mi TV firmware.".into());}},
            _=>return Err("This operation is not supported by this app version.".into())
        } Ok(())
    }

    fn confirmed_home(&self,s:&DeviceSnapshot)->Result<bool,String>{
        let current=s.home_component.as_deref().unwrap_or("");
        if !current.starts_with(&format!("{CUSTOM}/"))&&!current.starts_with(&format!("{ORIGINAL}/")){return Ok(false);}
        Ok(self.journal.list(Some(&s.identity.device_key))?.iter().any(|r|r.kind=="set_home"&&r.status=="confirmed"&&r.restore_status!="restored"&&r.fingerprint==s.identity.fingerprint&&normalize_component(&r.target)==normalize_component(current)))
    }

    pub fn apply(&self,serial:&str,plan:PlanPreview,progress:impl Fn(OperationProgress))->Result<ApplyResult,String>{
        if now_ms().saturating_sub(plan.created_at)>600_000||plan.created_at>now_ms()+30_000{return Err("This review expired. Review your selected changes again.".into());}
        let baseline=scanner::scan(&self.adb,serial)?;
        if baseline.identity.device_key!=plan.device_key||baseline.identity.fingerprint!=plan.fingerprint{return Err("The TV or firmware changed since review. Scan and review again.".into());}
        if plan.actions.is_empty()||plan.actions.len()>100{return Err("Choose between one and 100 changes.".into());}
        if plan.actions.iter().any(|a|a.kind=="set_home")&&plan.actions.len()!=1{return Err("Switch and confirm Home separately from other changes.".into());}
        let mut unique=BTreeSet::new();for a in &plan.actions{self.validate(&baseline,a)?;let key=if ["disable","enable","uninstall","force_stop"].contains(&a.kind.as_str()){format!("package:{}",a.target)}else{a.kind.clone()};if !unique.insert(key){return Err("Conflicting duplicate changes in the same plan.".into());}}
        self.journal.ensure_baseline(&baseline)?;
        self.cancelled.store(false,Ordering::SeqCst);
        let session=uuid::Uuid::new_v4().to_string();let total=plan.actions.len();let mut changes=vec![];let mut token=None;
        for (i,a) in plan.actions.iter().enumerate(){
            if self.cancelled.load(Ordering::SeqCst){break;}
            progress(OperationProgress{phase:"preparing".into(),message:format!("Saving original state: {}",label(a)),completed:i,total,change_id:None});
            let current=scanner::scan(&self.adb,serial)?;same_identity(&baseline.identity,&current.identity)?;self.validate(&current,a)?;
            if plan.preconditions.get(&resource_key(a))!=Some(&capture(&current,a)?.to_string()){return Err("An app or setting changed since you reviewed the plan. Review it again before applying.".into());}
            let id=uuid::Uuid::new_v4().to_string();let before=capture(&current,a)?;
            let mut r=ChangeRecord{id:id.clone(),session_id:session.clone(),device_key:current.identity.device_key.clone(),serial:serial.into(),model:current.identity.model.clone(),fingerprint:current.identity.fingerprint.clone(),user_id:current.identity.user_id,kind:a.kind.clone(),target:a.target.clone(),label:label(a),before_json:before.to_string(),after_json:"null".into(),status:"prepared".into(),restore_status:if ["force_stop","trim_cache"].contains(&a.kind.as_str())||(a.kind=="install_launcher"&&before["installed"]==true){"not_available"}else{"available"}.into(),error:None,created_at:now_ms(),updated_at:now_ms()};
            self.journal.save(&r)?;
            if let Err(e)=self.remote_record(serial,&r){r.status="failed".into();r.restore_status="not_available".into();r.error=Some(format!("No change made: could not save TV recovery record: {e}"));self.save(&mut r)?;changes.push(r);break;}
            progress(OperationProgress{phase:"applying".into(),message:label(a),completed:i,total,change_id:Some(id.clone())});
            match self.execute(serial,&current,a,&id){
                Ok(())=>{
                    match scanner::scan(&self.adb,serial).and_then(|s|{same_identity(&baseline.identity,&s.identity)?;let after=capture(&s,a)?;Ok((after,verify_action(&s,a)))}) {
                        Ok((after,verification))=>{r.after_json=after.to_string();if let Err(e)=verification{r.status="uncertain".into();r.error=Some(format!("The command returned, but the requested state was not fully verified: {e}"));self.save(&mut r)?;let _=self.remote_record(serial,&r);changes.push(r);break;}r.status=if a.kind=="set_home"{"awaiting_confirmation"}else{"applied"}.into();if a.kind=="set_home"{token=Some(id.clone());}self.save(&mut r)?;let _=self.remote_record(serial,&r);},
                        Err(e)=>{r.status="uncertain".into();r.error=Some(format!("The command ran, but verification was incomplete: {e}"));self.save(&mut r)?;changes.push(r);break;}
                    }
                },
                Err(e)=>{
                    r.status="uncertain".into();r.error=Some(e.clone());
                    // A timeout or nonzero exit can occur after a write. Preserve observed partial results.
                    if let Ok(observed)=scanner::scan(&self.adb,serial).and_then(|s|{same_identity(&baseline.identity,&s.identity)?;capture(&s,a)}){r.after_json=observed.to_string();if observed==before&&!matches!(a.kind.as_str(),"set_home"|"force_stop"|"trim_cache"){r.status="failed".into();r.restore_status="restored".into();}}
                    // DNS is one transaction: either both keys and validation succeed, or restore both.
                    if a.kind=="dns"||a.kind=="animations"||a.kind=="set_home" {
                        match self.restore_values(serial,&r,&before).and_then(|_|{let checked=scanner::scan(&self.adb,serial)?;same_identity(&baseline.identity,&checked.identity)?;if !restore_equal(&r.kind,&before,&capture(&checked,a)?){return Err("The recovery command returned, but read-back did not match the original state.".into());}Ok(())}){Ok(())=>{r.status="failed".into();r.restore_status="restored".into();r.error=Some(format!("Change failed; previous settings restored and verified: {e}"));},Err(re)=>r.error=Some(format!("{e}. Recovery still needs attention: {re}"))}
                    }
                    self.save(&mut r)?;changes.push(r);break;
                }
            }
            changes.push(r);
        }
        progress(OperationProgress{phase:"complete".into(),message:if self.cancelled.load(Ordering::SeqCst){"Stopped between changes. Completed changes remain in Restore Centre."}else{"Changes checked. Review results and test your TV."}.into(),completed:changes.len(),total,change_id:None});
        Ok(ApplyResult{session_id:session,changes,requires_home_confirmation:token.is_some(),home_token:token})
    }

    fn save(&self,r:&mut ChangeRecord)->Result<(),String>{r.updated_at=now_ms();self.journal.save(r)}
    fn remote_record(&self,serial:&str,r:&ChangeRecord)->Result<(),String>{
        let dir=self.root.join("data/recovery");fs::create_dir_all(&dir).map_err(|e|e.to_string())?;
        let path=dir.join(format!("{}.json",r.id));let data=serde_json::to_vec_pretty(r).map_err(|e|e.to_string())?;
        let mut f=fs::File::create(&path).map_err(|e|e.to_string())?;f.write_all(&data).map_err(|e|e.to_string())?;f.sync_all().map_err(|e|e.to_string())?;
        let remote=format!("/data/local/tmp/tvoptimizer/{}.json",r.id);
        self.adb.shell(serial,"mkdir -p /data/local/tmp/tvoptimizer")?;self.adb.push(serial,&path,&remote)?;
        let read=self.adb.shell(serial,&format!("cat {}",quote(&remote)))?;
        let parsed:Value=serde_json::from_str(&read).map_err(|_|"TV recovery record could not be verified")?;
        if parsed.get("id").and_then(Value::as_str)!=Some(&r.id){return Err("TV recovery record did not match".into());}Ok(())
    }

    fn execute(&self,serial:&str,s:&DeviceSnapshot,a:&ActionRequest,token:&str)->Result<(),String>{
        let user=s.identity.user_id;let pkg=quote(&a.target);
        match a.kind.as_str(){
            "disable"=>{check_pm(self.adb.shell(serial,&format!("pm disable-user --user {user} {pkg}"))?)?;},
            "enable"=>{if !package(s,&a.target)?.installed{check_pm(self.adb.shell(serial,&format!("cmd package install-existing --user {user} {pkg}"))?)?;}check_pm(self.adb.shell(serial,&format!("pm enable --user {user} {pkg}"))?)?;},
            "uninstall"=>{
                let p=package(s,&a.target)?;
                if !p.system{self.backup_apks(serial,p,token,user)?;}
                let out=self.adb.shell(serial,&format!("pm uninstall -k --user {user} {pkg}"))?;if out.trim()!="Success"{return Err(format!("App removal was not confirmed: {out}"));}
            },
            "force_stop"=>{self.adb.shell(serial,&format!("am force-stop --user {user} {pkg}"))?;let state=self.adb.shell(serial,&format!("dumpsys package {pkg}"))?;if !stopped_for_user(&state,user){return Err("Android did not confirm that the app stayed closed. It may have restarted automatically; no RAM saving is claimed.".into());}},
            "wallpaper_auto"=>crate::wallpapers::set_mode(&self.adb,serial,a.value=="1")?,
            "animations"=>{for k in animation_keys(){self.put_setting(serial,k,Some(&a.value))?;}},
            "dns"=>{
                let host=dns_hostname(&a.value).ok_or("Unsupported DNS provider")?;
                self.put_setting(serial,"global/private_dns_specifier",Some(host))?;self.put_setting(serial,"global/private_dns_mode",Some("hostname"))?;
                let mut validated=false;
                for _ in 0..8 {thread::sleep(Duration::from_millis(750));let output=self.adb.shell(serial,"dumpsys connectivity")?;if dns_network_validated(&output,host){validated=true;break;}}
                if !validated{return Err("The TV did not confirm a validated connection with this DNS provider.".into());}
                // A successful sinkholed ad-domain response alone cannot prove working Internet.
                let probe=self.adb.shell(serial,"ping -c 1 -W 3 www.google.com >/dev/null 2>&1; echo tvopt_dns_exit=$?")?;
                if !probe.contains("tvopt_dns_exit=0"){return Err("The ordinary Internet connectivity test failed. This network may block the probe; previous DNS will be restored.".into());}
            },
            "patchwall_startup"=>{self.put_setting(serial,"global/set_patchwall_default",Some("0"))?;},
            "install_launcher"=>{
                let path=self.launcher_path(&a.target,&a.value)?;
                if let Ok(p)=package(s,&a.target){if p.installed{self.backup_apks(serial,p,token,user)?;}}
                let p=path.to_str().ok_or("Invalid launcher path")?;let out=self.adb.with_timeout(150).run(&["-s",serial,"install","-r",p])?;if !out.lines().any(|l|l.trim()=="Success"){return Err(format!("Launcher installation was not confirmed: {out}"));}
            },
            "set_home"=>{
                self.arm_watchdog(serial,s,token)?;
                if a.value=="replace_google"&&package(s,GOOGLE).map(active).unwrap_or(false){check_pm(self.adb.shell(serial,&format!("pm disable-user --user {user} {}",quote(GOOGLE)))?)?;}
                check_pm(self.adb.shell(serial,&format!("cmd package set-home-activity --user {user} {}",quote(&a.target)))?)?;
                let out=self.adb.shell(serial,&format!("am start -W -n {}",quote(&a.target)))?;
                if !out.contains("Status: ok"){return Err(format!("The replacement Home did not start successfully: {out}"));}
                let resolved=self.adb.shell(serial,&format!("cmd package resolve-activity --brief --user {user} -a android.intent.action.MAIN -c android.intent.category.HOME"))?;
                if !resolved.lines().any(|l|normalize_component(l.trim())==normalize_component(&a.target)){return Err("The TV still resolves another Home screen. On Mi TV, explicitly choose replacement of Google Home and retry.".into());}
            },
            "trim_cache"=>{check_pm(self.adb.with_timeout(90).shell(serial,"pm trim-caches 256M internal")?)?;},
            _=>return Err("Unsupported action".into())
        }Ok(())
    }

    fn launcher_path(&self,id:&str,variant:&str)->Result<PathBuf,String>{
        if id==ORIGINAL{return crate::builder::original_path(&self.root).map(|(path,_)|path);}
        if id==CUSTOM&&!matches!(variant,"custom"|"build22"){return Err("Unknown SwiftOS version choice".into());}
        let name=if id==CUSTOM&&variant=="build22"{"liquid-glass.apk"}else if id==CUSTOM{SWIFTOS_APK_NAME}else if id==ORIGINAL{"original.apk"}else{return Err("Unknown launcher".into())};
        let bundled=self.root.join("launchers").join(name);
        #[cfg(not(feature = "swiftos-v3-candidate"))]
        let path=if bundled.is_file(){bundled}else if id==CUSTOM{self.root.join("../cleanyourtv-trial/next-liquid-update/CleanYourTV-MiTV-Liquid-21.apk")}else{return Err("The original launcher is missing. Extract the complete package.".into())};
        #[cfg(feature = "swiftos-v3-candidate")]
        let path=if bundled.is_file(){bundled}else{return Err("The SwiftOS v3 candidate APK is missing. Extract the complete package.".into())};
        if id==CUSTOM&&!hash_is_approved(){return Err("SwiftOS v3 is disabled until its reviewed APK hash is approved in this build.".into());}
        let bytes=fs::read(&path).map_err(|_|"The bundled launcher is missing")?;let hash=format!("{:x}",Sha256::digest(&bytes));
        let expected=if variant=="build22"{"ff6f0c1f93c086e803532f40b7b2165e1c820cd62e588da233fcdf96a4123c06"}else{SWIFTOS_HASH};
        if id==CUSTOM&&hash!=expected{return Err("The SwiftOS APK did not match its reviewed build hash.".into());}
        Ok(path)
    }

    fn backup_apks(&self,serial:&str,p:&PackageInfo,token:&str,user:u32)->Result<(),String>{
        // pm path is authoritative for base AND configuration/feature split APKs.
        let output=self.adb.shell(serial,&format!("pm path --user {user} {}",quote(&p.id)))?;
        let apk_paths=parse_apk_paths(&output)?;
        let dir=self.root.join("data/apks").join(token);fs::create_dir_all(&dir).map_err(|e|e.to_string())?;let mut manifest=vec![];
        for (i,remote) in apk_paths.iter().enumerate(){
            if !remote.starts_with('/')||!remote.ends_with(".apk"){return Err("An APK path was not valid".into());}
            let name=format!("{i}.apk");let local=dir.join(&name);self.adb.pull(serial,remote,&local)?;
            let data=fs::read(&local).map_err(|e|e.to_string())?;if data.len()<100{return Err("APK backup was incomplete".into());}
            manifest.push(json!({"file":name,"sha256":format!("{:x}",Sha256::digest(&data))}));
        }
        fs::write(dir.join("manifest.json"),serde_json::to_vec_pretty(&manifest).unwrap()).map_err(|e|e.to_string())?;Ok(())
    }

    fn put_setting(&self,serial:&str,key:&str,value:Option<&str>)->Result<(),String>{
        let (ns,name)=split_setting(key)?;let command=match value{Some(v)=>format!("settings put {ns} {name} {}",quote(v)),None=>format!("settings delete {ns} {name}")};self.adb.shell(serial,&command)?;Ok(())
    }

    fn arm_watchdog(&self,serial:&str,s:&DeviceSnapshot,token:&str)->Result<(),String>{
        // Prove a detached child survives closing the connection that launched it.
        let dir="/data/local/tmp/tvoptimizer";let probe=format!("{dir}/probe-{token}");
        let preflight=self.adb.shell(serial,"test -x /system/bin/nohup && test -x /system/bin/setsid && test -x /system/bin/sh && echo tvopt_recovery_tools")?;
        if preflight.trim()!="tvopt_recovery_tools"{return Err("This TV cannot run the detached recovery timer; switching Home is blocked.".into());}
        let proof=format!("sleep 1; echo survived > {}",quote(&probe));
        self.adb.shell(serial,&format!("/system/bin/nohup /system/bin/setsid /system/bin/sh -c {} </dev/null >/dev/null 2>&1 &",quote(&proof)))?;
        thread::sleep(Duration::from_millis(1400));let proof=self.adb.shell(serial,&format!("cat {}",quote(&probe)))?;
        if proof.trim()!="survived"{return Err("The recovery timer could not survive disconnecting its launch session.".into());}
        let before=home_state(s);let script=rollback_script(&before,s.identity.user_id,token,150)?;
        self.adb.shell(serial,&format!("/system/bin/nohup /system/bin/setsid /system/bin/sh -c {} </dev/null >{dir}/watchdog-{token}.log 2>&1 &",quote(&script)))?;
        let mut pid=String::new();for _ in 0..10 {if let Ok(candidate)=self.adb.shell(serial,&format!("cat {dir}/watchdog-{token}.pid")){if !candidate.trim().is_empty(){pid=candidate;break;}}thread::sleep(Duration::from_millis(100));}
        if !pid.trim().bytes().all(|b|b.is_ascii_digit())||pid.trim().is_empty(){return Err("The recovery timer did not provide a valid process identifier.".into());}
        let alive=self.adb.shell(serial,&format!("kill -0 {} 2>/dev/null && echo armed",pid.trim()))?;
        if alive.trim()!="armed"{return Err("The recovery timer did not start.".into());}Ok(())
    }

    pub fn confirm_home(&self,serial:&str,token:&str)->Result<ChangeRecord,String>{
        let mut r=self.journal.get(token)?;
        if r.kind!="set_home"||r.status!="awaiting_confirmation"||now_ms()-r.created_at>145_000{return Err("Home confirmation expired. Scan the TV; the recovery timer may have restored its previous Home.".into());}
        let s=scanner::scan(&self.adb,serial)?;guard_record(&r,&s)?;
        let after:Value=serde_json::from_str(&r.after_json).map_err(|e|e.to_string())?;
        if home_state(&s)!=after{return Err("Home settings changed before confirmation. Scan and restore the pending change.".into());}
        // The watchdog and confirmation acquire the same atomic directory lock. A late click can
        // never acknowledge a timer that has already started rollback.
        let result=self.adb.shell(serial,&home_confirmation_script(&r.id)?)?;
        if result.trim()!="tvopt_home_confirmed"{return Err("Could not acknowledge the TV recovery timer. It may already be restoring the previous Home.".into());}
        // Leave the acknowledged sleeping timer to exit naturally; no broad pkill pattern.
        r.status="confirmed".into();self.save(&mut r)?;let _=self.remote_record(serial,&r);Ok(r)
    }

    pub fn restore(&self,serial:&str,id:&str)->Result<ChangeRecord,String>{
        let mut r=self.journal.get(id)?;
        if r.restore_status=="restored"{return Ok(r);}
        if r.restore_status!="available"||r.status=="prepared"||r.status=="failed"{return Err("This record has no completed change that can be safely restored automatically. Review its details or export the recovery record.".into());}
        let s=scanner::scan(&self.adb,serial)?;guard_record(&r,&s)?;
        let action=ActionRequest{kind:r.kind.clone(),target:r.target.clone(),value:String::new()};
        let actual=capture(&s,&action)?;let after:Value=serde_json::from_str(&r.after_json).map_err(|e|e.to_string())?;let before:Value=serde_json::from_str(&r.before_json).map_err(|e|e.to_string())?;
        if ["disable","enable","uninstall"].contains(&r.kind.as_str()) {
            if scanner::is_protected(&r.target)||s.packages.iter().find(|p|p.id==r.target).map(|p|!restore_package_allowed(p)).unwrap_or(before["system"]==true&&!scanner::allowed_optional(&r.target)&&!is_launcher(&r.target)){return Err("This restore target is an essential or unreviewed system app. Imported records cannot bypass protected-app rules.".into());}
        }
        if r.kind=="set_home" {
            crate::recovery::validate_record(&r)?;
            let old=before["home_component"].as_str().ok_or("Missing previous Home")?;let old_id=old.split('/').next().unwrap_or("");
            if scanner::is_protected(old_id)||!s.packages.iter().any(|p|p.id==old_id&&p.installed&&p.is_launcher){return Err("The previous Home could not be verified as a usable launcher. Recovery was stopped to protect TV settings and system components.".into());}
        }
        if actual==before{if r.kind=="set_home"{self.adb.shell(serial,&home_defuse_script(&r.id)?)?;}r.restore_status="restored".into();self.save(&mut r)?;return Ok(r);}
        if after.is_null()||actual!=after{return Err("The TV's current state differs from the recorded result. Another app, update or person may have changed it. Restore was stopped to preserve those changes.".into());}
        if r.kind=="install_launcher"&&s.home_component.as_deref().map(|h|h.starts_with(&format!("{}/",r.target))).unwrap_or(false){return Err("Restore the previous Home screen before removing its current launcher.".into());}
        let before_ref=before.clone();self.restore_values(serial,&r,&before_ref)?;
        let checked=scanner::scan(&self.adb,serial)?;guard_record(&r,&checked)?;
        if !restore_equal(&r.kind,&before,&capture(&checked,&action)?){return Err("Restore ran, but the original state was not fully verified. Keep this recovery record.".into());}
        r.restore_status="restored".into();self.save(&mut r)?;let _=self.remote_record(serial,&r);Ok(r)
    }

    fn restore_values(&self,serial:&str,r:&ChangeRecord,before:&Value)->Result<(),String>{
        match r.kind.as_str(){
            "disable"|"enable"|"uninstall"=>{
                if !validate_package(&r.target)||scanner::is_protected(&r.target){return Err("Invalid protected app in restore record".into());}
                let installed=before.get("installed").and_then(Value::as_bool).ok_or("Invalid app recovery state")?;
                if installed {
                    if before.get("system").and_then(Value::as_bool)==Some(true){check_pm(self.adb.shell(serial,&format!("cmd package install-existing --user {} {}",r.user_id,quote(&r.target)))?)?;}
                    else if r.kind=="uninstall"{self.reinstall_backups(serial,r)?;}
                    let enabled=before.get("enabled").and_then(Value::as_u64).ok_or("Original enabled state was not saved")?;
                    self.adb.shell(serial,&enabled_command(&r.target,r.user_id,enabled as u32)?)?;
                }else if r.kind=="enable"&&before.get("system").and_then(Value::as_bool)==Some(true){
                    if let Some(enabled)=before.get("enabled").and_then(Value::as_u64){self.adb.shell(serial,&enabled_command(&r.target,r.user_id,enabled as u32)?)?;}
                    check_pm(self.adb.shell(serial,&format!("pm uninstall -k --user {} {}",r.user_id,quote(&r.target)))?)?;
                }else{return Err("Recovery record unexpectedly removes an app".into());}
            },
            "wallpaper_auto"=>crate::wallpapers::set_mode(&self.adb,serial,before["swiftos/auto_wallpaper"].as_str().ok_or("Missing previous wallpaper setting")?=="1")?,
            "animations"|"dns"|"patchwall_startup"=>{
                let values=before.as_object().ok_or("Invalid settings backup")?;
                for (key,value) in values {split_setting(key)?;self.put_setting(serial,key,value.as_str())?;}
            },
            "set_home"=>{
                let commands=home_restore_commands(before,r.user_id)?;
                let body=format!("if {commands}; then printf restored > /data/local/tmp/tvoptimizer/home-ok-{}; printf tvopt_home_restored; else printf tvopt_home_restore_failed; exit 25; fi",r.id);
                let result=self.adb.shell(serial,&home_lock_script(&r.id,&body)?)?;
                if result.trim_end().ends_with("tvopt_home_restore_failed"){return Err("The previous Home could not be restored. Its recovery timer remains armed.".into());}
            },
            "install_launcher"=>{
                if ![CUSTOM,ORIGINAL].contains(&r.target.as_str()){return Err("Unknown launcher restore".into());}
                if before.get("installed").and_then(Value::as_bool)==Some(true){return Err("An installed launcher was updated. Android can reject downgrades, and uninstalling would lose private settings. The previous APK is saved for a deliberate compatible update.".into());}
                let out=self.adb.shell(serial,&format!("pm uninstall -k --user {} {}",r.user_id,quote(&r.target)))?;check_pm(out)?;
            },
            _=>return Err("This change cannot be restored automatically".into())
        }Ok(())
    }
    fn reinstall_backups(&self,serial:&str,r:&ChangeRecord)->Result<(),String>{
        let dir=self.root.join("data/apks").join(&r.id);let data=fs::read(dir.join("manifest.json")).map_err(|_|"APK backup is missing")?;let entries:Vec<Value>=serde_json::from_slice(&data).map_err(|_|"Invalid APK backup manifest")?;
        if entries.is_empty()||entries.len()>100{return Err("Invalid APK backup list".into());}
        let mut paths=vec![];for entry in entries{let name=entry["file"].as_str().ok_or("Invalid APK backup filename")?;if name.contains('/')||name.contains('\\')||!name.ends_with(".apk"){return Err("Invalid APK backup path".into());}let path=dir.join(name);let bytes=fs::read(&path).map_err(|e|e.to_string())?;if format!("{:x}",Sha256::digest(&bytes))!=entry["sha256"].as_str().unwrap_or(""){return Err("APK backup integrity check failed".into());}paths.push(path.to_string_lossy().into_owned());}
        let mut args=vec!["-s",serial,if paths.len()==1{"install"}else{"install-multiple"},"-r"];for path in &paths{args.push(path);}check_pm(self.adb.with_timeout(150).run(&args)?)?;Ok(())
    }
}

fn restorable(r:&ChangeRecord)->bool{r.restore_status=="available"&&!matches!(r.status.as_str(),"prepared"|"failed")&&!matches!(r.kind.as_str(),"trim_cache"|"force_stop")}
fn restore_all_plan(original:&DeviceSnapshot,current:&DeviceSnapshot,records:&[ChangeRecord])->Result<RestoreAllPreview,String>{
    same_identity(&original.identity,&current.identity)?;
    let mut virtual_state=current.clone();let mut ids=Vec::new();let mut labels=Vec::new();let mut skipped=Vec::new();let mut preconditions=std::collections::BTreeMap::new();
    // Journal membership supplies local changes only, newest first. Project each
    // undo into a copy so overlapping launcher/package dependencies are checked.
    for r in records {
        if r.restore_status=="restored"||r.status=="failed"{continue;}
        if r.restore_status=="needs_review"||r.status=="prepared"{return Err("An interrupted change needs review before Restore all. Reconnect and rescan, then inspect Change history.".into());}
        if r.status=="awaiting_confirmation"&&now_ms().saturating_sub(r.created_at)<160_000{return Err("Finish or let the current Home recovery timer expire before Restore all.".into());}
        if !restorable(r){skipped.push(format!("{} — no complete Undo is available.",r.label));continue;}
        guard_record(r,&virtual_state)?;crate::recovery::validate_record(r)?;
        let action=ActionRequest{kind:r.kind.clone(),target:r.target.clone(),value:String::new()};
        preconditions.insert(r.id.clone(),capture(current,&action)?.to_string());
        let before:Value=serde_json::from_str(&r.before_json).map_err(|e|e.to_string())?;let after:Value=serde_json::from_str(&r.after_json).map_err(|e|e.to_string())?;
        if ["disable","enable","uninstall"].contains(&r.kind.as_str())&&virtual_state.packages.iter().find(|p|p.id==r.target).map(|p|!restore_package_allowed(p)).unwrap_or(before["system"]==true&&!scanner::allowed_optional(&r.target)&&!is_launcher(&r.target)){return Err("A protected system service cannot be included in Restore all.".into());}
        let actual=capture(&virtual_state,&action)?;
        if actual!=before&&(after.is_null()||after!=actual){return Err(format!("{} changed outside the recorded sequence. Restore all stopped before making changes; review this item individually.",r.label));}
        if r.kind=="install_launcher"&&before["installed"]==false&&virtual_state.home_component.as_deref().map(|home|home.starts_with(&format!("{}/",r.target))).unwrap_or(false){return Err("The current Home must be restored before its launcher can be removed.".into());}
        project_restore(&mut virtual_state,r,&before)?;ids.push(r.id.clone());labels.push(r.label.clone());
    }
    for r in records.iter().filter(|r|ids.contains(&r.id)) {
        let action=ActionRequest{kind:r.kind.clone(),target:r.target.clone(),value:String::new()};
        let starting=if r.kind=="wallpaper_auto"&&!original.settings.contains_key("swiftos/auto_wallpaper"){let first=records.iter().rev().find(|prior|prior.kind=="wallpaper_auto").ok_or("Missing first wallpaper setting")?;serde_json::from_str(&first.before_json).map_err(|e|e.to_string())?}else{capture(original,&action)?};let projected=capture(&virtual_state,&action)?;
        let matches=if r.kind=="set_home"{starting["home_component"]==projected["home_component"]&&starting["patchwall_startup"]==projected["patchwall_startup"]&&starting["launchers"].as_object().map(|launchers|launchers.iter().all(|(id,state)|projected["launchers"][id]==*state)).unwrap_or(false)}else{restore_equal(&r.kind,&starting,&projected)};
        if !matches{return Err("The recorded change sequence does not return this item to its starting restore point. A manual change may have happened before optimization. Review the individual restore records; Restore all made no changes.".into());}
    }
    Ok(RestoreAllPreview{device_key:current.identity.device_key.clone(),fingerprint:current.identity.fingerprint.clone(),baseline_at:original.scanned_at,change_ids:ids,labels,skipped,created_at:now_ms(),preconditions})
}
fn project_restore(s:&mut DeviceSnapshot,r:&ChangeRecord,before:&Value)->Result<(),String>{
    match r.kind.as_str(){
        "disable"|"enable"|"uninstall"|"install_launcher"=>{
            if let Some(p)=s.packages.iter_mut().find(|p|p.id==r.target){p.installed=before["installed"].as_bool().ok_or("Invalid saved installation state")?;p.enabled=before["enabled"].as_u64().map(|n|n as u32);p.version_code=before["version_code"].as_u64().unwrap_or(p.version_code);}
            else if before["installed"]==true{return Err("A previously installed app is missing from the scan. Review its recovery record individually.".into());}
        },
        "wallpaper_auto"=>{s.settings.insert("swiftos/auto_wallpaper".into(),before["swiftos/auto_wallpaper"].as_str().map(str::to_string));},"animations"|"dns"|"patchwall_startup"=>{for (key,value) in before.as_object().ok_or("Invalid saved settings")?{split_setting(key)?;s.settings.insert(key.clone(),value.as_str().map(str::to_string));}},
        "set_home"=>{
            let home=before["home_component"].as_str().ok_or("Missing previous Home")?;let id=home.split('/').next().unwrap_or("");
            if !s.packages.iter().any(|p|p.id==id&&p.installed&&p.is_launcher&&!scanner::is_protected(id)){return Err("The starting Home could not be verified as a usable launcher.".into());}
            for (id,value) in before["launchers"].as_object().ok_or("Missing launcher states")?{let p=s.packages.iter_mut().find(|p|p.id==*id).ok_or("A recorded launcher is missing from this scan")?;p.installed=value["installed"].as_bool().ok_or("Invalid launcher state")?;p.enabled=value["enabled"].as_u64().map(|v|v as u32);}
            s.home_component=Some(home.into());s.settings.insert("global/set_patchwall_default".into(),before["patchwall_startup"].as_str().map(str::to_string));
        },
        _=>return Err("This change has no complete Undo".into())
    }Ok(())
}

fn animation_keys()->[&'static str;3]{["global/window_animation_scale","global/transition_animation_scale","global/animator_duration_scale"]}
fn stopped_for_user(output:&str,user:u32)->bool{output.lines().map(str::trim).find(|line|line.starts_with(&format!("User {user}:"))).map(|line|line.split_whitespace().any(|token|token=="stopped=true")).unwrap_or(false)}
fn dns_keys()->[&'static str;2]{["global/private_dns_mode","global/private_dns_specifier"]}
fn split_setting(key:&str)->Result<(&str,&str),String>{
    let pair=key.split_once('/').ok_or("Invalid setting key")?;
    if !animation_keys().contains(&key)&&!dns_keys().contains(&key)&&key!="global/set_patchwall_default"{return Err("This setting is not writable through the optimizer".into());}Ok(pair)
}
fn label(a:&ActionRequest)->String{match a.kind.as_str(){"disable"=>format!("Disable {}",a.target),"enable"=>format!("Enable {}",a.target),"uninstall"=>format!("Remove {} for this TV user",a.target),"force_stop"=>format!("Temporarily close {}",a.target),"animations"=>format!("Animation speed {}×",a.value),"dns"=>"Change DNS ad protection".into(),"patchwall_startup"=>"Use preferred Home at Mi TV startup".into(),"set_home"=>"Change preferred Home screen".into(),"install_launcher"=>if a.target==CUSTOM{"Install SwiftOS"}else{"Install original CleanYourTV"}.into(),"trim_cache"=>"Free rebuildable cache".into(),_=>a.kind.clone()}}
fn package_state(p:Option<&PackageInfo>)->Value{match p{Some(p)=>json!({"installed":p.installed,"enabled":p.enabled,"system":p.system,"version_code":p.version_code}),None=>json!({"installed":false,"enabled":null,"system":false,"version_code":0})}}
fn home_state(s:&DeviceSnapshot)->Value{
        let mut launchers=serde_json::Map::new();for id in [GOOGLE,PATCHWALL,PATCHWALL_LEGACY,CUSTOM,ORIGINAL]{if let Some(p)=s.packages.iter().find(|p|p.id==id){launchers.insert(id.into(),json!({"installed":p.installed,"enabled":p.enabled}));}}
    if let Some((old_id,_))=s.home_component.as_deref().and_then(|c|c.split_once('/')){if let Some(p)=s.packages.iter().find(|p|p.id==old_id){launchers.insert(old_id.into(),json!({"installed":p.installed,"enabled":p.enabled}));}}
    json!({"home_component":s.home_component,"launchers":launchers,"patchwall_startup":s.settings.get("global/set_patchwall_default").cloned().unwrap_or(None)})
}
fn capture(s:&DeviceSnapshot,a:&ActionRequest)->Result<Value,String>{
    match a.kind.as_str(){
        "disable"|"enable"|"uninstall"|"install_launcher"=>Ok(package_state(s.packages.iter().find(|p|p.id==a.target))),
        "set_home"=>Ok(home_state(s)),
        "animations"|"dns"|"patchwall_startup"|"wallpaper_auto"=>{let keys:Vec<&str>=match a.kind.as_str(){"wallpaper_auto"=>vec!["swiftos/auto_wallpaper"],"animations"=>animation_keys().into(),"dns"=>dns_keys().into(),_=>vec!["global/set_patchwall_default"]};let mut map=serde_json::Map::new();for k in keys{require_setting(s,k)?;map.insert(k.into(),json!(s.settings[k]));}Ok(Value::Object(map))},
        "force_stop"|"trim_cache"=>Ok(Value::Null),_=>Err("Unknown recovery resource".into())
    }
}
fn verify_action(s:&DeviceSnapshot,a:&ActionRequest)->Result<(),String>{
    let okay=match a.kind.as_str(){
        "disable"=>package(s,&a.target).map(|p|p.installed&&p.enabled==Some(3)).unwrap_or(false),"enable"=>package(s,&a.target).map(active).unwrap_or(false),"uninstall"=>!s.packages.iter().any(|p|p.id==a.target&&p.installed),
        "install_launcher"=>package(s,&a.target).map(active).unwrap_or(false),
        "set_home"=>s.home_component.as_deref().map(|h|normalize_component(h)==normalize_component(&a.target)).unwrap_or(false),
        "animations"=>animation_keys().iter().all(|k|s.settings.get(*k).and_then(|v|v.as_deref()).and_then(|v|v.parse::<f32>().ok())==a.value.parse::<f32>().ok()),
        "dns"=>s.settings.get("global/private_dns_mode").and_then(|v|v.as_deref())==Some("hostname")&&s.settings.get("global/private_dns_specifier").and_then(|v|v.as_deref())==dns_hostname(&a.value),
        "wallpaper_auto"=>s.settings.get("swiftos/auto_wallpaper").and_then(|v|v.as_deref())==Some(a.value.as_str()),"patchwall_startup"=>s.settings.get("global/set_patchwall_default").and_then(|v|v.as_deref())==Some("0"),"force_stop"|"trim_cache"=>true,_=>false
    };if okay{Ok(())}else{Err("The TV did not match the requested state after the operation.".into())}
}
fn restore_equal(kind:&str,before:&Value,current:&Value)->bool{
    if ["disable","enable","uninstall"].contains(&kind){before["installed"]==current["installed"]&&before["enabled"]==current["enabled"]}else if kind=="install_launcher"&&before["installed"]==false{current["installed"]==false}else{before==current}
}
fn interrupted_state(kind:&str,before:&Value,after:&Value,actual:&Value)->&'static str{
    if restore_equal(kind,before,actual){"original"}else if !after.is_null()&&after==actual{"recorded"}else{"review"}
}
fn guard_record(r:&ChangeRecord,s:&DeviceSnapshot)->Result<(),String>{if r.device_key!=s.identity.device_key||r.user_id!=s.identity.user_id{return Err("This restore point belongs to a different TV or Android user.".into());}if r.fingerprint!=s.identity.fingerprint{return Err("TV firmware changed since this backup. Restore needs a new compatibility review.".into());}Ok(())}
fn check_pm(output:String)->Result<String,String>{let lower=output.to_lowercase();if lower.contains("failure")||lower.contains("error:")||lower.contains("unknown command")||lower.contains("not found")||lower.contains("exception"){Err(format!("The TV rejected the operation: {}",output.chars().take(600).collect::<String>()))}else{Ok(output)}}
fn enabled_command(id:&str,user:u32,state:u32)->Result<String,String>{if !validate_package(id){return Err("Invalid recovery package".into());}let cmd=match state{0=>"default-state",1=>"enable",2=>"disable",3=>"disable-user",4=>"disable-until-used",_=>return Err("Unknown original enabled state".into())};Ok(format!("pm {cmd} --user {user} {}",quote(id)))}
fn normalize_component(c:&str)->String{if let Some((p,a))=c.split_once('/'){if a.starts_with('.') {return format!("{p}/{p}{a}");}}c.into()}
fn home_restore_commands(before:&Value,user:u32)->Result<String,String>{
    let home=before["home_component"].as_str().filter(|h|valid_component(h)).ok_or("Invalid original Home component")?;
    let launchers=before["launchers"].as_object().ok_or("Invalid Home recovery state")?;let home_id=home.split('/').next().unwrap();let mut commands=vec![];
    // Re-enable the previous launcher before returning Home. Restore all other exact states after launch.
    let state=launchers.get(home_id).filter(|v|v["installed"]==true).and_then(|v|v["enabled"].as_u64()).filter(|n|*n<=4).ok_or("Original Home's exact app state was not saved")?as u32;
    commands.push(enabled_command(home_id,user,if matches!(state,0|1){state}else{1})?);
    commands.push(format!("cmd package set-home-activity --user {user} {}",quote(home)));
    commands.push(format!("am start -W -n {}",quote(home)));
    for (id,v) in launchers{if !is_launcher(id)&&id!=home_id{return Err("Unrecognized app in Home recovery".into());}if v["installed"]==true {if let Some(state)=v["enabled"].as_u64(){commands.push(enabled_command(id,user,state as u32)?);}}}
    let startup=before.get("patchwall_startup").and_then(Value::as_str);commands.push(match startup{Some(v)=>format!("settings put global set_patchwall_default {}",quote(v)),None=>"settings delete global set_patchwall_default".into()});
    Ok(commands.join(" && "))
}
fn rollback_script(before:&Value,user:u32,token:&str,seconds:u32)->Result<String,String>{
    if uuid::Uuid::parse_str(token).is_err()||!(30..=300).contains(&seconds){return Err("Invalid recovery timer".into());}
    let root="/data/local/tmp/tvoptimizer";
    let commands=home_restore_commands(before,user)?;
    let lock=format!("{root}/home-lock-{token}");
    Ok(format!("echo $$ > {root}/watchdog-{token}.pid; sleep {seconds} & wait; tries=0; until mkdir {lock} 2>/dev/null; do tries=$((tries+1)); if [ \"$tries\" -ge 20 ]; then printf lock_failed > {root}/watchdog-{token}.result; exit 26; fi; sleep 1; done; trap {} EXIT; trap 'exit 27' HUP INT TERM; [ -f {root}/home-ok-{token} ] && exit 0; if {commands}; then printf restored > {root}/watchdog-{token}.result; else printf failed > {root}/watchdog-{token}.result; exit 28; fi",quote(&format!("rmdir {lock} 2>/dev/null"))))
}

fn home_lock_script(token:&str,body:&str)->Result<String,String>{
    if uuid::Uuid::parse_str(token).is_err(){return Err("Invalid Home recovery identifier".into());}
    let lock=format!("/data/local/tmp/tvoptimizer/home-lock-{token}");
    Ok(format!("if mkdir {lock} 2>/dev/null; then trap {} EXIT; trap 'exit 27' HUP INT TERM; {body}; else printf tvopt_home_busy; exit 23; fi",quote(&format!("rmdir {lock} 2>/dev/null"))))
}
fn home_confirmation_script(token:&str)->Result<String,String>{let root="/data/local/tmp/tvoptimizer";home_lock_script(token,&format!("if [ -f {root}/watchdog-{token}.result ]; then printf tvopt_home_expired; exit 24; fi; printf confirmed > {root}/home-ok-{token}; printf tvopt_home_confirmed"))}
fn home_defuse_script(token:&str)->Result<String,String>{home_lock_script(token,&format!("printf restored > /data/local/tmp/tvoptimizer/home-ok-{token}; printf tvopt_home_defused"))}
fn parse_apk_paths(output:&str)->Result<Vec<String>,String>{
    let mut paths=vec![];for line in output.lines(){let p=line.trim().strip_prefix("package:").ok_or("Unexpected APK path response; backup was blocked")?;if !p.starts_with('/')||!p.ends_with(".apk")||p.contains('\0')||p.split('/').any(|s|s==".."){return Err("Invalid app APK path; backup was blocked".into());}if !paths.iter().any(|known|known==p){paths.push(p.to_string());}}
    if paths.is_empty()||paths.len()>100{return Err("Could not locate all app APKs; uninstall/update was blocked before making a change.".into());}Ok(paths)
}
fn dns_network_validated(output:&str,hostname:&str)->bool{
    let active=output.lines().find_map(|line|line.trim().strip_prefix("Active default network:").map(str::trim)).filter(|id|!id.is_empty()&&id.bytes().all(|b|b.is_ascii_digit()));
    let Some(active)=active else{return false;};
    // NetworkAgentInfo is the current network summary; old event logs and listener requests are not evidence.
    output.lines().filter(|line|line.trim_start().starts_with("NetworkAgentInfo{")).any(|line|{
        let server=line.split_once("PrivateDnsServerName:").and_then(|(_,rest)|rest.split_whitespace().next());
        let capabilities=line.split_once("Capabilities:").map(|(_,rest)|rest.split("Unwanted:").next().unwrap_or(""));
        line.contains(&format!("network{{{active}}}"))&&(line.contains("UsePrivateDns: true")||line.contains("PrivateDnsActive: true"))&&server==Some(hostname)&&capabilities.map(|c|c.split(|ch:char|!ch.is_ascii_uppercase()&&ch!='_').any(|c|c=="VALIDATED")).unwrap_or(false)
    })
}

#[cfg(test)]mod tests{
    use super::*;
    #[test]fn close_verification_uses_the_selected_user_and_requires_explicit_stopped_state(){
        let output="  User 0: installed=true stopped=false enabled=0\n  User 10: installed=true stopped=true enabled=0\n";
        assert!(!stopped_for_user(output,0));assert!(stopped_for_user(output,10));
        assert!(!stopped_for_user(output,1));assert!(!stopped_for_user("User 0: installed=true",0));
    }
    #[test]fn wallpaper_restore_uses_its_first_known_state_when_an_older_baseline_had_no_protocol(){
        let original=restore_snapshot();let mut current=original.clone();
        current.settings.insert("swiftos/auto_wallpaper".into(),Some("1".into()));
        let record=undo_record("wallpaper_auto","swiftos/auto_wallpaper",json!({"swiftos/auto_wallpaper":"0"}),json!({"swiftos/auto_wallpaper":"1"}));
        let plan=restore_all_plan(&original,&current,&[record.clone()]).unwrap();assert_eq!(plan.change_ids,vec![record.id.clone()]);
        current.settings.insert("swiftos/auto_wallpaper".into(),Some("unexpected".into()));
        assert!(restore_all_plan(&original,&current,&[record]).is_err());
    }
    fn restore_snapshot()->DeviceSnapshot{serde_json::from_value(json!({"identity":{"serial":"tv","device_key":"a".repeat(64),"model":"MiTV-AXSO1","manufacturer":"Xiaomi","android_version":"9","api":28,"abi":"arm","fingerprint":"firmware","user_id":0},"scanned_at":100,"profile":null,"packages":[],"home_component":null,"settings":{},"memory_total_kb":null,"memory_available_kb":null,"storage_free_kb":null,"processes":"","inspections":[]})).unwrap()}
    fn undo_record(kind:&str,target:&str,before:Value,after:Value)->ChangeRecord{ChangeRecord{id:uuid::Uuid::new_v4().to_string(),session_id:uuid::Uuid::new_v4().to_string(),device_key:"a".repeat(64),serial:"tv".into(),model:"MiTV-AXSO1".into(),fingerprint:"firmware".into(),user_id:0,kind:kind.into(),target:target.into(),label:kind.into(),before_json:before.to_string(),after_json:after.to_string(),status:"applied".into(),restore_status:"available".into(),error:None,created_at:200,updated_at:200}}
    fn dns_state(host:&str)->Value{json!({"global/private_dns_mode":"hostname","global/private_dns_specifier":host})}
    #[test]fn restore_all_does_not_claim_a_later_manual_state_is_the_starting_point(){let mut original=restore_snapshot();original.settings=serde_json::from_value(dns_state("starting.example")).unwrap();let mut current=original.clone();current.settings=serde_json::from_value(dns_state("dns.adguard-dns.com")).unwrap();let r=undo_record("dns","private_dns",dns_state("later-manual.example"),dns_state("dns.adguard-dns.com"));assert!(restore_all_plan(&original,&current,&[r]).unwrap_err().contains("starting restore point"));}
    #[test]fn restore_all_reverses_multiple_changes_to_the_preexisting_dns(){let mut original=restore_snapshot();let original_dns=dns_state("x-hagezi-pro.freedns.controld.com");original.settings=serde_json::from_value(original_dns.clone()).unwrap();let mut current=original.clone();current.settings=serde_json::from_value(dns_state("x-hagezi-ultimate.freedns.controld.com")).unwrap();let first=undo_record("dns","private_dns",original_dns,dns_state("dns.adguard-dns.com"));let second=undo_record("dns","private_dns",dns_state("dns.adguard-dns.com"),dns_state("x-hagezi-ultimate.freedns.controld.com"));let plan=restore_all_plan(&original,&current,&[second.clone(),first.clone()]).unwrap();assert_eq!(plan.change_ids,vec![second.id.clone(),first.id.clone()]);for r in [&second,&first]{project_restore(&mut current,r,&serde_json::from_str(&r.before_json).unwrap()).unwrap();}assert_eq!(current.settings,original.settings);assert_eq!(plan.baseline_at,100);}
    #[test]fn restore_all_checks_manual_conflicts_and_firmware_before_writes(){let original=restore_snapshot();let mut current=original.clone();current.settings=serde_json::from_value(dns_state("manual.example")).unwrap();let r=undo_record("dns","private_dns",dns_state("old.example"),dns_state("dns.adguard-dns.com"));assert!(restore_all_plan(&original,&current,&[r]).unwrap_err().contains("outside the recorded sequence"));current.identity.fingerprint="new firmware".into();assert!(restore_all_plan(&original,&current,&[]).is_err());}
    #[test]fn restore_all_rejects_manually_reenabled_and_updated_app_during_preview(){let mut original=restore_snapshot();original.packages.push(PackageInfo{id:"com.example.player".into(),label:"Player".into(),installed:true,enabled:Some(0),system:false,version_name:"1".into(),version_code:1,apk_paths:vec![],is_launcher:false,protected:false,reason:String::new(),purpose:String::new()});original.settings=serde_json::from_value(dns_state("starting.example")).unwrap();let before=package_state(Some(&original.packages[0]));let mut disabled=original.packages[0].clone();disabled.enabled=Some(3);let app=undo_record("disable",&disabled.id,before,package_state(Some(&disabled)));let mut current=original.clone();current.packages[0].version_code=2;current.settings=serde_json::from_value(dns_state("dns.adguard-dns.com")).unwrap();let dns=undo_record("dns","private_dns",dns_state("starting.example"),dns_state("dns.adguard-dns.com"));let observed=serde_json::to_value(&current).unwrap();assert!(restore_all_plan(&original,&current,&[dns,app]).unwrap_err().contains("outside the recorded sequence"));assert_eq!(serde_json::to_value(&current).unwrap(),observed);}
    #[test]fn restore_all_projects_launcher_dependencies_before_removal(){let mut original=restore_snapshot();for id in [GOOGLE,PATCHWALL,CUSTOM]{original.packages.push(PackageInfo{id:id.into(),label:id.into(),installed:true,enabled:Some(0),system:id!=CUSTOM,version_name:String::new(),version_code:22,apk_paths:vec![],is_launcher:true,protected:false,reason:String::new(),purpose:String::new()});}original.home_component=Some(format!("{GOOGLE}/.MainActivity"));let home_before=home_state(&original);let mut current=original.clone();current.home_component=Some(format!("{CUSTOM}/.MainActivity"));current.packages[0].enabled=Some(3);let home_after=home_state(&current);let home=undo_record("set_home",&format!("{CUSTOM}/.MainActivity"),home_before,home_after);let patch_before=package_state(Some(&current.packages[1]));current.packages[1].enabled=Some(3);let patch=undo_record("disable",PATCHWALL,patch_before,package_state(Some(&current.packages[1])));let install=undo_record("install_launcher",CUSTOM,json!({"installed":false,"enabled":null,"system":false,"version_code":0}),package_state(Some(&current.packages[2])));original.packages[2].installed=false;original.packages[2].enabled=None;original.packages[2].version_code=0;let plan=restore_all_plan(&original,&current,&[patch.clone(),home.clone(),install.clone()]).unwrap();assert_eq!(plan.change_ids,vec![patch.id,home.id,install.id.clone()]);assert!(restore_all_plan(&original,&current,&[install]).is_err());}
    #[test]fn restore_all_lists_irreversible_limits_and_blocks_unreconciled_changes(){let current=restore_snapshot();let mut r=undo_record("trim_cache","all",Value::Null,Value::Null);r.restore_status="not_available".into();let plan=restore_all_plan(&current,&current,&[r.clone()]).unwrap();assert!(plan.change_ids.is_empty());assert_eq!(plan.skipped.len(),1);r.restore_status="needs_review".into();assert!(restore_all_plan(&current,&current,&[r]).is_err());}
    #[test]fn restore_cannot_bypass_unknown_system_protection(){let mut p=PackageInfo{id:"com.vendor.critical".into(),label:"System service".into(),installed:true,enabled:Some(0),system:true,version_name:String::new(),version_code:1,apk_paths:vec![],is_launcher:false,protected:true,reason:String::new(),purpose:String::new()};assert!(!restore_package_allowed(&p));p.protected=false;assert!(!restore_package_allowed(&p));p.system=false;assert!(restore_package_allowed(&p));p.id="com.google.android.apps.inputmethod.hindi".into();assert!(!restore_package_allowed(&p));}
    #[test]fn uninstall_warning_uses_fresh_disabled_state_and_exact_recovery_limits(){let mut p=PackageInfo{id:"com.miui.tv.analytics".into(),label:"Xiaomi TV analytics".into(),installed:true,enabled:Some(3),system:true,version_name:String::new(),version_code:1,apk_paths:vec![],is_launcher:false,protected:false,reason:String::new(),purpose:String::new()};let warning=uninstall_warning(&p);assert!(warning.contains("Xiaomi TV analytics is already disabled"));assert!(warning.contains("may not improve speed or RAM"));assert!(warning.contains("factory APK stays"));p.enabled=None;assert!(!uninstall_warning(&p).contains("already disabled"));p.system=false;assert!(uninstall_warning(&p).contains("verified APK backup"));}
    #[test]fn interruption_reconciliation_never_attributes_unknown_or_later_states(){let before=json!({"global/private_dns_mode":null,"global/private_dns_specifier":null});let after=json!({"global/private_dns_mode":"hostname","global/private_dns_specifier":"dns.adguard-dns.com"});assert_eq!(interrupted_state("dns",&before,&Value::Null,&before),"original");assert_eq!(interrupted_state("dns",&before,&after,&after),"recorded");assert_eq!(interrupted_state("dns",&before,&Value::Null,&after),"review");assert_eq!(interrupted_state("dns",&before,&after,&json!({"global/private_dns_mode":"off"})),"review");}
    #[test]fn dns_profiles_are_fixed_and_distinct(){assert!(dns_hostname("x;id").is_none());assert!(dns_hostname("off").is_none());assert_ne!(dns_hostname("pro"),dns_hostname("proplus"));assert_eq!(dns_hostname("ultimate"),Some("x-hagezi-ultimate.freedns.controld.com"));}
    #[test]fn preserve_all_enabled_states(){for (n,cmd)in [(0,"default-state"),(1,"enable"),(2,"disable"),(3,"disable-user"),(4,"disable-until-used")]{assert!(enabled_command(GOOGLE,0,n).unwrap().contains(&format!("pm {cmd} --user 0")));}assert!(enabled_command("com.a;id",0,0).is_err());assert!(enabled_command(GOOGLE,0,8).is_err());}
    #[test]fn fallback_and_component_injection_rejected(){assert!(!valid_component("com.a/.Home;reboot"));assert!(!valid_component("x;id/.Home"));assert!(valid_component("com.a/.MainActivity"));assert_eq!(normalize_component("com.a/.Main"),"com.a/com.a.Main");}
    #[test]fn home_restore_preserves_original_default_and_patchwall_setting(){let v=json!({"home_component":"com.google.android.tvlauncher/.MainActivity","launchers":{GOOGLE:{"installed":true,"enabled":0},PATCHWALL:{"installed":true,"enabled":0},CUSTOM:{"installed":true,"enabled":3}},"patchwall_startup":null});let s=home_restore_commands(&v,0).unwrap();assert!(s.starts_with("pm default-state"));assert!(s.contains("settings delete global set_patchwall_default"));assert!(s.find("am start").unwrap()<s.find("pm disable-user").unwrap());}
    #[test]fn timer_ack_is_checked_before_recovery(){let v=json!({"home_component":"com.gathin.tvhome/.MainActivity","launchers":{CUSTOM:{"installed":true,"enabled":1}},"patchwall_startup":"0"});let t=uuid::Uuid::new_v4().to_string();let s=rollback_script(&v,0,&t,150).unwrap();assert!(s.contains("sleep 150 & wait"));assert!(s.find("home-ok").unwrap()<s.find("pm enable").unwrap());assert!(rollback_script(&v,0,"bad;id",150).is_err());}
    #[test]fn only_whitelisted_settings_writable(){assert!(split_setting("global/private_dns_mode").is_ok());assert!(split_setting("secure/enabled_accessibility_services").is_err());assert!(split_setting("global/foo;id").is_err());}
    #[test]fn split_apk_backups_include_configuration_and_feature_splits(){let paths=parse_apk_paths("package:/data/app/com.example/base.apk\npackage:/data/app/com.example/split_config.arm64_v8a.apk\npackage:/data/app/com.example/split_feature.apk\n").unwrap();assert_eq!(paths.len(),3);assert!(parse_apk_paths("").is_err());assert!(parse_apk_paths("package:/data/app/../wrong.apk").is_err());assert!(parse_apk_paths("Error: package inaccessible").is_err());}
    #[test]fn dns_validation_ignores_old_logs_and_other_networks(){let current="Active default network: 100\nCurrent Networks:\n NetworkAgentInfo{network{100} lp{UsePrivateDns: true PrivateDnsServerName: dns.adguard-dns.com Domains: null} nc{Capabilities: INTERNET&VALIDATED Unwanted: }}\n";assert!(dns_network_validated(current,"dns.adguard-dns.com"));assert!(!dns_network_validated(current,"x-hagezi-pro.freedns.controld.com"));let stale="Active default network: 100\n NetworkAgentInfo{network{101} UsePrivateDns: true PrivateDnsServerName: dns.adguard-dns.com Capabilities: INTERNET&VALIDATED Unwanted: }\n 2026 log network{100} UsePrivateDns: true PrivateDnsServerName: dns.adguard-dns.com Capabilities: VALIDATED\n";assert!(!dns_network_validated(stale,"dns.adguard-dns.com"));assert!(!dns_network_validated(&current.replace("UsePrivateDns: true","UsePrivateDns: false"),"dns.adguard-dns.com"));}
    #[test]fn home_confirmation_and_rollback_share_atomic_lock(){let token=uuid::Uuid::new_v4().to_string();let v=json!({"home_component":"com.gathin.tvhome/.MainActivity","launchers":{CUSTOM:{"installed":true,"enabled":0}},"patchwall_startup":null});let timer=rollback_script(&v,0,&token,150).unwrap();let confirm=home_confirmation_script(&token).unwrap();let lock=format!("home-lock-{token}");assert!(timer.contains(&format!("mkdir /data/local/tmp/tvoptimizer/{lock}")));assert!(confirm.contains(&format!("mkdir /data/local/tmp/tvoptimizer/{lock}")));assert!(confirm.find("watchdog-").unwrap()<confirm.find("printf confirmed").unwrap());assert!(timer.contains("else printf failed"));assert!(home_defuse_script(&token).unwrap().contains("printf restored"));}
    #[test]fn generic_previous_home_preserves_manifest_default(){let v=json!({"home_component":"com.vendor.home/.HomeActivity","launchers":{"com.vendor.home":{"installed":true,"enabled":0},CUSTOM:{"installed":true,"enabled":1}},"patchwall_startup":null});let commands=home_restore_commands(&v,0).unwrap();assert!(commands.starts_with("pm default-state --user 0 'com.vendor.home'"));let missing=json!({"home_component":"com.vendor.home/.HomeActivity","launchers":{},"patchwall_startup":null});assert!(home_restore_commands(&missing,0).is_err());}
}
