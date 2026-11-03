import { inflateSync } from 'node:zlib';
import { createHash } from 'node:crypto';
// Bounded RGB/RGBA PNG decoding for CDP screenshots, no image dependency.
export function decodePng(bytes) {
  if(bytes.subarray(0,8).toString('hex')!=='89504e470d0a1a0a')throw Error('Not PNG');
  let width,height,channels;const chunks=[];
  for(let at=8;at+12<=bytes.length;){const size=bytes.readUInt32BE(at),type=bytes.toString('ascii',at+4,at+8);if(at+12+size>bytes.length)throw Error('Truncated PNG');
    if(type==='IHDR'){width=bytes.readUInt32BE(at+8);height=bytes.readUInt32BE(at+12);const depth=bytes[at+16],color=bytes[at+17];channels=color===6?4:color===2?3:0;if(depth!==8||!channels||bytes[at+20]!==0)throw Error('Need noninterlaced RGB8/RGBA8 PNG');}
    if(type==='IDAT')chunks.push(bytes.subarray(at+8,at+8+size));at+=12+size;
  }
  if(!width||!height||width*height>34000000)throw Error('PNG size outside bound');
  const stride=width*channels,raw=inflateSync(Buffer.concat(chunks),{maxOutputLength:(stride+1)*height});
  if(raw.length!==(stride+1)*height)throw Error('Invalid PNG scanline length');
  const pixels=Buffer.alloc(stride*height);
  const paeth=(a,b,c)=>{const p=a+b-c,aa=Math.abs(p-a),bb=Math.abs(p-b),cc=Math.abs(p-c);return aa<=bb&&aa<=cc?a:bb<=cc?b:c;};
  for(let y=0;y<height;y++){const type=raw[y*(stride+1)];if(type>4)throw Error('Unknown PNG filter');for(let x=0;x<stride;x++){const a=x>=channels?pixels[y*stride+x-channels]:0,b=y?pixels[(y-1)*stride+x]:0,c=y&&x>=channels?pixels[(y-1)*stride+x-channels]:0;pixels[y*stride+x]=(raw[y*(stride+1)+1+x]+([0,a,b,Math.floor((a+b)/2),paeth(a,b,c)][type]))&255;}}
  return {width,height,channels,pixels};
}
function circle(a,b,c){const d=2*(a[0]*(b[1]-c[1])+b[0]*(c[1]-a[1])+c[0]*(a[1]-b[1]));if(Math.abs(d)<1e-6)return null;
  const aa=a[0]**2+a[1]**2,bb=b[0]**2+b[1]**2,cc=c[0]**2+c[1]**2;
  const x=(aa*(b[1]-c[1])+bb*(c[1]-a[1])+cc*(a[1]-b[1]))/d,y=(aa*(c[0]-b[0])+bb*(a[0]-c[0])+cc*(b[0]-a[0]))/d;return {x,y,radius:Math.hypot(x-a[0],y-a[1])};}
export function pictureEvidence({width,height,channels,pixels}) {
  const red=[],redMask=new Uint8Array(width*height),mask=new Uint8Array(width*height),step=Math.max(1,Math.floor(width/960));
  for(let y=0;y<height;y++)for(let x=0;x<width;x++){const p=(y*width+x)*channels,r=pixels[p],g=pixels[p+1],b=pixels[p+2];
    // Accept red/gold boundary candidates. Colour alone never anchors a fitted
    // circle to the outer world instead of a pit or other drawing.
    if(r>35&&r>g*1.2&&r>b*1.8)redMask[y*width+x]=1;
    if(x>width*.05&&x<width*.8&&y>height*.08&&y<height*.9&&Math.max(r,g,b)>85&&Math.max(r,g,b)-Math.min(r,g,b)>55)mask[y*width+x]=1;
  }
  // Some stock clients tint the entire region outside the arena. Fit its edge,
  // not arbitrary samples from that filled area; ignore the screenshot border.
  for(let y=step;y<height-step;y+=step)for(let x=step;x<width-step;x+=step){const i=y*width+x;
    if(redMask[i]&&[i-step,i+step,i-step*width,i+step*width].some(n=>!redMask[n]))red.push([x,y]);
  }
  let pelletCandidates=0;const queue=[];
  for(let i=0;i<mask.length;i++)if(mask[i]){mask[i]=0;queue.length=0;queue.push(i);let head=0,minX=width,maxX=0,minY=height,maxY=0;
    while(head<queue.length){const p=queue[head++],x=p%width,y=Math.floor(p/width);minX=Math.min(minX,x);maxX=Math.max(maxX,x);minY=Math.min(minY,y);maxY=Math.max(maxY,y);
      for(const n of [x?p-1:-1,x+1<width?p+1:-1,y?p-width:-1,y+1<height?p+width:-1])if(n>=0&&mask[n]){mask[n]=0;queue.push(n);}}
    const w=maxX-minX+1,h=maxY-minY+1;
    if(queue.length>=2&&queue.length<=160&&w<=18&&h<=18&&Math.max(w,h)<=3*Math.min(w,h))pelletCandidates++;
  }
  const points=red.filter((_,i)=>i%Math.max(1,Math.floor(red.length/3500))===0);
  let best=null,state=7;const rand=()=>{state=(Math.imul(state,1664525)+1013904223)>>>0;return state/4294967296;};
  for(let i=0;i<500&&points.length>=3;i++){const c=circle(points[Math.floor(rand()*points.length)],points[Math.floor(rand()*points.length)],points[Math.floor(rand()*points.length)]);if(!c||c.radius<height*.12||c.radius>height*5)continue;
    let support=0;const sectors=new Set();for(const [x,y]of points){if(Math.abs(Math.hypot(x-c.x,y-c.y)-c.radius)<=Math.max(2,c.radius*.012)){support++;sectors.add(Math.floor((Math.atan2(y-c.y,x-c.x)+Math.PI)*48/(2*Math.PI)));}}
    const score=sectors.size*20+support;if(sectors.size>=24&&(!best||score>best.score))best={...c,support,sectors:sectors.size,score};
  }
  return {boundary:best,wholeMap:Boolean(best&&best.sectors>=42&&best.x-best.radius>=-2&&best.y-best.radius>=-2&&best.x+best.radius<=width+2&&best.y+best.radius<=height+2),
    pelletCandidates,gameCanvasVisible:pelletCandidates>0,sceneEvidence:'image',sceneConfidence:'presence-only; exact count unverified',viewport:[width,height]};
}
export function pngEvidence(bytes){return {...pictureEvidence(decodePng(bytes)),imageSha256:createHash('sha256').update(bytes).digest('hex')};}
