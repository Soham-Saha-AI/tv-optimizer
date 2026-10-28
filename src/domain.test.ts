import {describe,it,expect} from 'vitest';
import {addSelection,matchesArea,isRestorable,isDisabled,packagePurpose,isActionableRecommendation,optimizationsFound,applyOutcome} from './domain';
import type {ActionKind,BasketItem} from './models';
const item=(kind:ActionKind,target='app.a',value=''):BasketItem=>({action:{kind,target,value},title:kind,impact:'',restore:'',risk:'low'});
const recommendation=(extra:Partial<import('./models').Recommendation>={}):import('./models').Recommendation=>({id:'dns',area:'Ads & DNS',title:'Use AdGuard standard filtering',description:'',evidence:'',impact:'',restore:'',risk:'low',supported:true,already_applied:false,action:{kind:'dns',target:'private_dns',value:'adguard'},...extra});
describe('available changes and existing optimizations',()=>{
 it('recommends only supported changes that are not already active',()=>{expect(isActionableRecommendation(recommendation(),true)).toBe(true);expect(isActionableRecommendation(recommendation({already_applied:true}),true)).toBe(false);expect(isActionableRecommendation(recommendation({supported:false}),true)).toBe(false);expect(isActionableRecommendation(recommendation({action:{kind:'force_stop',target:'app.a',value:''}}),true)).toBe(false)});
 it('keeps launcher changes unavailable until guarded Home is confirmed and leaves normal animation control to Performance',()=>{const rec=recommendation({action:{kind:'disable',target:'com.mitv.tvhome.atv',value:''}});expect(isActionableRecommendation(rec,false)).toBe(false);expect(isActionableRecommendation(rec,true)).toBe(true);expect(isActionableRecommendation(recommendation({action:{kind:'animations',target:'all',value:'1'}}),true)).toBe(false)});
 it('reports observed optimizations separately and never infers an unreadable enabled state',()=>{const snapshot={packages:[{id:'app.a',label:'Usage reports',installed:true,enabled:3,system:true},{id:'app.b',label:'Unknown service',installed:true,enabled:null,system:true}],settings:{'global/window_animation_scale':'0.5','global/transition_animation_scale':'0.5','global/animator_duration_scale':'0.5'},home_component:null} as unknown as import('./models').DeviceSnapshot;const notes=optimizationsFound(snapshot,[]);expect(notes).toContain('Usage reports is disabled.');expect(notes).toContain('System animations use reduced timing.');expect(notes.some(note=>note.includes('Unknown service'))).toBe(false)});
});
describe('honest execution feedback',()=>{
 const result=(status:string,error:string|null=null):import('./models').ApplyResult=>({changes:[{status,error} as import('./models').ChangeRecord],session_id:'s',requires_home_confirmation:false,home_token:null});
 it('does not report a failed or uncertain command as success',()=>{expect(applyOutcome(result('failed','TV rejected the command'),1).error).toContain('TV rejected the command');expect(applyOutcome(result('uncertain'),1).succeeded).toBe(0);expect(applyOutcome(result('applied'),1).notice).toContain('verified on your TV')});
 it('clearly reports a cancelled or partially completed batch',()=>{expect(applyOutcome(result('applied'),3).notice).toContain('Stopped after 1 of 3');expect(applyOutcome({...result('applied'),changes:[]},1).notice).toContain('Stopped after 0 of 1')});
});
describe('review basket',()=>{
 it('replaces incompatible choices for one app',()=>{const result=addSelection([item('disable')],item('uninstall'));expect(result).toHaveLength(1);expect(result[0].action.kind).toBe('uninstall')});
 it('cannot close and remove the same app together',()=>{expect(addSelection([item('force_stop')],item('uninstall'))).toHaveLength(1)});
 it('selects one DNS tier at a time',()=>{const result=addSelection([item('dns','private_dns','pro')],item('dns','private_dns','ultimate'));expect(result).toHaveLength(1);expect(result[0].action.value).toBe('ultimate')});
 it('isolates Home handover until confirmation',()=>{expect(addSelection([item('disable')],item('set_home','com.home/.Main'))).toHaveLength(1);expect(addSelection([item('set_home','com.home/.Main')],item('dns','private_dns','pro')).every(i=>i.action.kind!=='set_home')).toBe(true)});
 it('keeps Google and PatchWall choices separate',()=>{expect(addSelection([item('disable','com.google.android.tvlauncher')],item('disable','com.mitv.tvhome.atv'))).toHaveLength(2)});
});
describe('restore and categories',()=>{
 it('never offers Undo for temporary or already-restored operations',()=>{expect(isRestorable('applied','not_available','trim_cache')).toBe(false);expect(isRestorable('applied','restored','dns')).toBe(false);expect(isRestorable('failed','available','disable')).toBe(false);expect(isRestorable('uncertain','available','dns')).toBe(true)});
 it('matches backend category spelling',()=>{expect(matchesArea('Ads & DNS','privacy')).toBe(true);expect(matchesArea('Animations','performance')).toBe(true);expect(matchesArea('Launchers','launcher')).toBe(true);expect(matchesArea('Privacy','apps')).toBe(false)});
});

import {deviceConnectionMessage,isConnectionError,appStatus} from './domain';
import {isNewerRelease} from './domain';
describe('launcher update offers',()=>{it('offers newer stable versions without treating omitted trailing zeroes as updates',()=>{expect(isNewerRelease('v2.7.0','2.7')).toBe(false);expect(isNewerRelease('v2.8.0','2.7')).toBe(true);expect(isNewerRelease('v2.10.0','2.9')).toBe(true);expect(isNewerRelease('v2.6.0','2.7')).toBe(false);expect(isNewerRelease('unknown','2.7')).toBe(false)})});
describe('beginner connection feedback',()=>{
 it('explains permission, offline and ready states without assuming unknown connections are ready',()=>{
  expect(deviceConnectionMessage('unauthorized')).toContain('Allow on the TV');
  expect(deviceConnectionMessage('offline')).toContain('not responding');
  expect(deviceConnectionMessage('device')).toContain('Ready to scan');
  expect(deviceConnectionMessage('unknown')).toContain('not ready');
 });
 it('recognizes actual transport loss but not an unavailable app or unsupported setting',()=>{
  expect(isConnectionError('error: device offline')).toBe(true);
  expect(isConnectionError('error: device not found')).toBe(true);
  expect(isConnectionError('Connection refused')).toBe(true);
  expect(isConnectionError('No devices/emulators found')).toBe(true);
  expect(isConnectionError('Package not found: optional.app')).toBe(false);
  expect(isConnectionError('Private DNS could not be verified')).toBe(false);
 });
 it('preserves unknown state rather than describing it as disabled',()=>{
  expect(appStatus({installed:true,enabled:null} as import('./models').PackageInfo)).toBe('Could not check');
  expect(appStatus({installed:false,enabled:0} as import('./models').PackageInfo)).toBe('Uninstalled');
 });
});

describe('ADB connection details',()=>{
 it('recognizes a missing explicitly targeted TV even when its address is quoted',()=>{
  expect(isConnectionError("error: device '192.0.2.10:5555' not found")).toBe(true);
  expect(isConnectionError('error: device "usb-tv-serial" unauthorized')).toBe(true);
 });
});

describe('explained app removal',()=>{
 it('distinguishes disabled apps from unknown and removed states',()=>{
  const pkg={installed:true,enabled:3} as import('./models').PackageInfo;
  expect(isDisabled(pkg)).toBe(true);
  expect(isDisabled({...pkg,enabled:0})).toBe(false);
  expect(isDisabled({...pkg,enabled:null})).toBe(false);
  expect(isDisabled({...pkg,installed:false})).toBe(false);
 });
 it('shows reviewed purpose and never guesses an unknown service purpose',()=>{
  const pkg={id:'com.example.app',label:'Known app',purpose:'TV usage reports.',reason:'Older explanation. Restore details.'} as import('./models').PackageInfo;
  expect(packagePurpose(pkg)).toBe('TV usage reports.');
  expect(packagePurpose({...pkg,purpose:undefined})).toBe('Older explanation.');
  expect(packagePurpose({...pkg,purpose:undefined,label:pkg.id})).toContain('Purpose not yet verified');
  expect(packagePurpose()).toContain('Purpose not yet verified');
 });
});
