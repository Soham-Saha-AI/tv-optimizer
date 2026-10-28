export type WallpaperWeather='any'|'clear'|'cloudy'|'fog'|'drizzle'|'rain'|'snow'|'thunderstorm'|'unknown';
export type WallpaperTime='all'|'morning'|'day'|'evening'|'night';
export interface WallpaperImage {name:string;weather:WallpaperWeather;time:WallpaperTime;bytes:number}
export interface WallpaperStatus {supported:boolean;reason:string;folder:string;enabled:boolean|null;storage_granted:boolean;images:WallpaperImage[];limit:number}
export interface PreparedWallpaper {name:string;weather:WallpaperWeather;time:WallpaperTime;jpeg_base64:string}
async function invoke<T>(command:string,args:Record<string,unknown>){if(!('__TAURI_INTERNALS__' in window))throw new Error('Open the portable Windows app to manage TV photos.');const {invoke}=await import('@tauri-apps/api/core');return invoke<T>(command,args)}
export const wallpaperApi={status:(serial:string)=>invoke<WallpaperStatus>('wallpaper_status',{serial}),upload:(serial:string,deviceKey:string,images:PreparedWallpaper[])=>invoke<{uploaded:number;folder:string}>('upload_wallpapers',{serial,deviceKey,images})};

// Read bounded headers before decoding, so a small compressed file cannot request
// an unlimited bitmap allocation on the PC. The TV receives only resized JPEGs.
export function wallpaperDimensions(bytes:Uint8Array):{width:number;height:number}{
 const view=new DataView(bytes.buffer,bytes.byteOffset,bytes.byteLength);let width=0,height=0;const text=(start:number,length:number)=>String.fromCharCode(...bytes.subarray(start,start+length));
 if(bytes.length>=24&&bytes[0]===137&&text(1,3)==='PNG'){width=view.getUint32(16);height=view.getUint32(20);let offset=8;while(offset+12<=bytes.length){const length=view.getUint32(offset),kind=text(offset+4,4);if(kind==='acTL')throw new Error('Animated PNGs are not supported. Choose a still photo.');if(kind==='IDAT')break;offset+=12+length;}}
 else if(bytes.length>=4&&bytes[0]===255&&bytes[1]===216){let offset=2;while(offset+4<=bytes.length){if(bytes[offset]!==255){offset++;continue}while(bytes[offset]===255)offset++;const marker=bytes[offset++];if(marker===217||marker===218)break;if(marker===216||marker===1||marker>=208&&marker<=215)continue;if(offset+2>bytes.length)break;const length=view.getUint16(offset);if(length<2||offset+length>bytes.length)break;if([192,193,194,195,197,198,199,201,202,203,205,206,207].includes(marker)&&length>=7){height=view.getUint16(offset+3);width=view.getUint16(offset+5);break}offset+=length;}}
 else if(bytes.length>=30&&text(0,4)==='RIFF'&&text(8,4)==='WEBP'){const kind=text(12,4);if(kind==='VP8X'){if(bytes[20]&2)throw new Error('Animated WebP images are not supported. Choose a still photo.');const u24=(offset:number)=>bytes[offset]|bytes[offset+1]<<8|bytes[offset+2]<<16;width=u24(24)+1;height=u24(27)+1}else if(kind==='VP8L'&&bytes[20]===47){const packed=view.getUint32(21,true);width=(packed&16383)+1;height=((packed>>>14)&16383)+1}else if(kind==='VP8 '&&bytes[23]===157&&bytes[24]===1&&bytes[25]===42){width=view.getUint16(26,true)&16383;height=view.getUint16(28,true)&16383}}
 if(!width||!height||width>16384||height>16384||width*height>64_000_000)throw new Error('Choose a valid still JPG, PNG or WebP photo under 64 megapixels. Photos with unusually large metadata may need to be exported again.');
 return {width,height};
}
export function wallpaperSize(width:number,height:number){const scale=Math.min(1,1920/width,1080/height);return {width:Math.max(1,Math.round(width*scale)),height:Math.max(1,Math.round(height*scale))}}
export async function prepareWallpaper(file:File,weather:WallpaperWeather,time:WallpaperTime,index:number){
 if(file.size>20*1024*1024)throw new Error(`${file.name}: choose a photo smaller than 20 MB.`);
 wallpaperDimensions(new Uint8Array(await file.slice(0,1024*1024).arrayBuffer()));
 const bitmap=await createImageBitmap(file,{imageOrientation:'from-image'});const canvas=document.createElement('canvas');
 try{const size=wallpaperSize(bitmap.width,bitmap.height);canvas.width=size.width;canvas.height=size.height;const context=canvas.getContext('2d',{alpha:false});if(!context)throw new Error('Photo preparation is unavailable on this PC.');context.fillStyle='#10131a';context.fillRect(0,0,size.width,size.height);context.drawImage(bitmap,0,0,size.width,size.height);
  let blob:Blob|null=null;for(const quality of [.85,.72,.6]){blob=await new Promise<Blob|null>(resolve=>canvas.toBlob(resolve,'image/jpeg',quality));if(blob&&blob.size<=2*1024*1024)break}if(!blob||blob.size>2*1024*1024)throw new Error(`${file.name}: this photo could not be reduced safely. Choose a smaller photo.`);
  const bytes=new Uint8Array(await blob.arrayBuffer());let binary='';for(let offset=0;offset<bytes.length;offset+=16384)binary+=String.fromCharCode(...bytes.subarray(offset,offset+16384));const stem=file.name.replace(/\.[^.]+$/,'').normalize('NFKD').replace(/[^a-zA-Z0-9_-]+/g,'-').replace(/^-+|-+$/g,'').slice(0,42)||'photo';
  return {image:{name:`${stem}-${Date.now().toString(36)}-${index+1}.jpg`,weather,time,jpeg_base64:btoa(binary)} as PreparedWallpaper,bytes:blob.size,width:size.width,height:size.height};
 }finally{bitmap.close();canvas.width=1;canvas.height=1}
}
