import type {ActionRequest,BasketItem,PackageInfo,Recommendation,DeviceSnapshot,ApplyResult} from './models';
export const actionKey=(action:ActionRequest)=>`${action.kind}:${action.target}`;
export const isEnabled=(pkg:PackageInfo)=>pkg.installed&&(pkg.enabled===0||pkg.enabled===1);
export const isDisabled=(pkg:PackageInfo)=>pkg.installed&&pkg.enabled!==null&&[2,3,4].includes(pkg.enabled);
export function packagePurpose(pkg?:PackageInfo){
 const purpose=pkg?.purpose?.trim();if(purpose)return purpose;
 // Older saved scans have a reviewed explanation instead of a separate purpose field.
 if(pkg&&pkg.label!==pkg.id&&pkg.reason?.trim()&&!/^(No validated|App origin could not|Unknown)/i.test(pkg.reason))return pkg.reason.trim().split(/(?<=\.)\s/)[0];
 return 'Purpose not yet verified. Check this app on the TV before changing it.';
}
export const appStatus=(pkg:PackageInfo)=>!pkg.installed?'Uninstalled':pkg.enabled===null?'Could not check':isEnabled(pkg)?'Enabled':'Disabled';
export const isRestorable=(status:string,restore:string,kind:string)=>['applied','verified','uncertain','confirmed','awaiting_confirmation'].includes(status)&&restore==='available'&&!['force_stop','trim_cache'].includes(kind);
export const formatSize=(kb:number|null)=>kb===null?'Could not check':kb>=1048576?`${(kb/1048576).toFixed(1)} GB`:`${Math.round(kb/1024)} MB`;
export function addSelection(items:BasketItem[],item:BasketItem){
 const kinds=new Set(['disable','enable','uninstall','force_stop']);
 // Home handover must be confirmed before any other mutation can be selected with it.
 if(item.action.kind==='set_home')return [item];
 return [...items.filter(existing=>existing.action.kind!=='set_home'&&(existing.action.target!==item.action.target||!((kinds.has(existing.action.kind)&&kinds.has(item.action.kind))||existing.action.kind===item.action.kind))),item];
}
export function matchesArea(area:string,filter:string){
 const a=area.toLowerCase();
 if(filter==='all')return true;
 if(filter==='apps')return a==='apps';
 if(filter==='launcher')return a==='launchers';
 if(filter==='performance')return ['memory','animations','storage'].includes(a);
 if(filter==='privacy')return ['privacy','ads & dns'].includes(a);
 return a===filter.toLowerCase();
}
export function orderedItems(items:BasketItem[]){const order:Record<string,number>={install_launcher:0,set_home:1,patchwall_startup:2,enable:3,disable:4,uninstall:5,animations:6,dns:7,force_stop:8,trim_cache:9};return items.map((item,index)=>({item,index})).sort((a,b)=>(order[a.item.action.kind]??99)-(order[b.item.action.kind]??99)||a.index-b.index).map(value=>value.item)}

export function isActionableRecommendation(item:Recommendation,homeConfirmed:boolean){
 if(!item.supported||item.already_applied||item.area==='User apps'||['force_stop','uninstall','enable'].includes(item.action.kind))return false;
 if(item.action.kind==='animations'&&item.action.value==='1')return false;
 if(item.action.kind==='disable'&&['com.google.android.tvlauncher','com.mitv.tvhome.atv','com.mitv.tvhome'].includes(item.action.target)&&!homeConfirmed)return false;
 return true;
}
export function optimizationsFound(snapshot:DeviceSnapshot,recommendations:Recommendation[]){
 const notes:string[]=[];
 for(const pkg of snapshot.packages){
  if(isDisabled(pkg))notes.push(`${pkg.label} is disabled.`);
  else if(!pkg.installed&&pkg.system&&recommendations.some(item=>item.action.kind==='disable'&&item.action.target===pkg.id&&item.already_applied))notes.push(`${pkg.label} is removed for this TV user.`);
 }
 const scales=['global/window_animation_scale','global/transition_animation_scale','global/animator_duration_scale'].map(key=>snapshot.settings[key]);
 if(scales.every(value=>value!==null&&value!==undefined&&value.trim()!==''&&Number.isFinite(Number(value))&&Number(value)>=0&&Number(value)<1))notes.push(scales.every(value=>Number(value)===0)?'System animations are off.':'System animations use reduced timing.');
 const dns=recommendations.find(item=>item.action.kind==='dns'&&item.already_applied);if(dns)notes.push(`${dns.title.replace(/^Use /,'').replace(/ filtering$/,'')} DNS filtering is configured.`);
 const home=snapshot.packages.find(pkg=>pkg.installed&&snapshot.home_component?.startsWith(`${pkg.id}/`));
 if(home&&['com.gathin.tvhome','com.gathin.tvhome.original'].includes(home.id))notes.push(`${home.id==='com.gathin.tvhome.original'?'Original CleanYourTV':'SwiftOS'} is your preferred Home screen.`);
 return [...new Set(notes)];
}
export function applyOutcome(result:ApplyResult,requested:number){
 const succeeded=result.changes.filter(change=>['applied','verified','confirmed','awaiting_confirmation'].includes(change.status)).length;
 const problems=result.changes.filter(change=>!['applied','verified','confirmed','awaiting_confirmation'].includes(change.status));
 const error=problems.length?`${problems.length} ${problems.length===1?'change could':'changes could'} not be verified. ${problems[0].error??'Review the result in Restore Centre before retrying.'}`:'';
 const notice=result.requires_home_confirmation?'Press Home on your TV remote, then confirm the new screen works.':problems.length?`${succeeded} of ${requested} changes verified. Open Change history for details.`:succeeded<requested?`Stopped after ${succeeded} of ${requested} changes. Unfinished choices were not reported as successful.`:requested===1?'The change was verified on your TV.':`All ${succeeded} changes were verified on your TV.`;
 return {succeeded,error,notice};
}

export function deviceConnectionMessage(state:string){
 if(state==='device')return 'Ready to scan. No TV settings will change.';
 if(state==='unauthorized')return 'Waiting for you to choose Allow on the TV.';
 if(state==='offline')return 'TV is not responding. Keep it awake and check the connection.';
 return 'Connection is not ready. Try Find my TV again.';
}
export function isConnectionError(error:string){return /device(?:\s+['"][^'"\r\n]+['"])?\s+(?:offline|not found|unauthorized)|no devices?\/emulators?|not connected|connection\s+(?:closed|refused|reset|lost)|cannot connect|unable to connect|failed to connect|network is unreachable|transport.*(?:closed|error)/i.test(error)}
export function isNewerRelease(latest:string,installed:string){const parse=(value:string)=>{const match=/^v?(\d+)\.(\d+)(?:\.(\d+))?/i.exec(value.trim());return match?[Number(match[1]),Number(match[2]),Number(match[3]??0)]:null};const a=parse(latest),b=parse(installed);if(!a||!b)return false;for(let i=0;i<3;i++){if(a[i]!==b[i])return a[i]>b[i]}return false}


