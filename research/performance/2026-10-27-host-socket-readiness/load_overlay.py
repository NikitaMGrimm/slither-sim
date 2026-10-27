"""Matched drain-only stock/legacy handshake and controls overlay."""
from pathlib import Path
import sys
path=Path(sys.argv[1]);t=path.read_text()
def replace(before,after):
    global t
    assert t.count(before)==1,(before,t.count(before));t=t.replace(before,after)
replace('        state: ClientState,','        state: ClientState,\n        legacy: bool,\n        drain_dead: bool,\n        next_ping: f64,')
replace('                state: ClientState::new(),','''                state: ClientState::new(),
                legacy: match std::env::var("STUDY_LEGACY").as_deref() { Ok("all")=>true,Ok("half")=>id%2==0,_=>false },
                drain_dead: false,
                next_ping: 0.,''')
replace('                Link::Open(_) => match self.state.status() {\n                    ClientStatus::Playing => 4,\n                    ClientStatus::Dead => 5,\n                    _ => 3,\n                },','                Link::Open(_) => if self.drain_dead {5} else if self.playing {4} else {3},')
replace('            self.state.record_sent(bytes, at)?;','            // Drain-only study load: do not build a client world.\n            let _ = at;')
replace('.and_then(|()| self.send(ctx, &[99, 0], now));','.and_then(|()| self.send(ctx, &[99, if self.legacy {4} else {0}], now));')
replace('            self.playing = false;\n            self.boost = None;','            self.playing = false;\n            self.drain_dead = false;\n            self.next_ping = now + 1.;\n            self.boost = None;')
replace('if self.state.status() == ClientStatus::Dead {','if self.drain_dead {')
start=t.index('        fn receive(&mut self, ctx: &mut Ctx, bytes: &[u8], at: f64) -> Result<(), String> {')
end=t.index('        fn end(&mut self, ctx:',start)
t=t[:start]+'''        fn receive(&mut self, ctx: &mut Ctx, bytes: &[u8], at: f64) -> Result<(), String> {
            let packets=split_frame(bytes)?;
            ctx.count("framesIn");ctx.add("bytesIn",bytes.len() as u64);
            if ctx.in_window(at) {self.frames+=1;self.bytes+=bytes.len() as u64;ctx.window.frames+=1;ctx.window.bytes_in+=bytes.len() as u64;}
            if let Some(prompt)=packets.iter().find(|p|p[0]==b'6') {
                if self.challenged || self.admitted {return Err("repeated challenge".into());}
                self.challenged=true;
                let reply=if self.legacy {legacy_reply(&prompt[1..])?}else{challenge::solve_challenge(&prompt[1..])?};
                self.send(ctx,&reply,at)?;
                let login=if self.legacy {
                    let name=self.nickname.as_bytes();let mut p=vec![b's',12,(self.id%9) as u8,name.len() as u8];p.extend(name);p.extend([0,255]);p
                }else{login_packet(&self.nickname,(self.id%9) as u8)?};
                self.send(ctx,&login,at)?;self.login_sent_at=Some(at);ctx.count("loginsSent");
            }
            if let Some(setup)=packets.iter().find(|p|p[0]==b'a') {
                if setup.get(23)!=Some(&if self.legacy {13}else{19}) {return Err("wrong output protocol".into());}
                self.admitted=true;self.ever_admitted=true;self.failures=0;ctx.count("admitted");
                if ctx.in_window(at){self.admissions+=1;}
                if let Some(start)=self.login_sent_at.take(){ctx.spawn_waits.push((self.id,self.connections,start,at,true));}
            }
            if !self.playing && packets.iter().any(|p|p[0]==b's' && p.len()>7) {
                self.playing=true;ctx.count("lifeStarts");self.next_control=at+self.unit()*CONTROL_INTERVAL;
            }
            if packets.iter().any(|p|p[0]==b'v'){self.playing=false;self.drain_dead=true;}
            Ok(())
        }
        fn tick(&mut self,ctx:&mut Ctx,now:f64)->Result<(),String>{
            if self.admitted && now>=self.next_ping {self.send(ctx,&[251],now)?;self.next_ping=now+1.;}
            if !self.playing || now<self.next_control{return Ok(());}
            let late=(now-self.next_control)*1000.;
            if ctx.in_window(now){ctx.lateness.push(late as f32);ctx.second_late_max=ctx.second_late_max.max(late);}
            self.next_control+=CONTROL_INTERVAL;
            if self.next_control<=now{self.next_control=now+CONTROL_INTERVAL;ctx.count("controlSkips");}
            // Same open-loop movement/boost schedule for both wire protocols.
            let heading=(now*0.65+self.id as f64*0.37+0.6*(now*0.23+self.id as f64).sin()).rem_euclid(TAU);
            let boost=(now+self.id as f64*0.37).rem_euclid(8.)<3.;
            if boost && ctx.in_window(now){ctx.count("boostControls");}
            for packet in action_packets(Action{heading,boost},self.boost)?{self.send(ctx,&packet,now)?;}
            self.boost=Some(boost);ctx.count("controls");Ok(())
        }

'''+t[end:]
point=t.index('    struct Client {')
t=t[:point]+'''    fn legacy_reply(encoded:&[u8])->Result<Vec<u8>,String>{
        if encoded.len()>512 || encoded.len()%2!=0{return Err("bad legacy challenge length".into());}
        let mut key=23u8;let mut source=Vec::new();
        for pair in encoded.chunks_exact(2){let mut byte=0;
            for letter in pair {if !letter.is_ascii_lowercase(){return Err("bad challenge encoding".into());}
                let nibble=(letter-b'a'+26-key)%26;if nibble>15{return Err("bad challenge nibble".into());}
                byte=(byte<<4)|nibble;key=(key+17)%26;
            }source.push(byte);
        }
        let text=source.split(|&b|b==39).nth(1).ok_or("unsupported legacy challenge")?;
        if text.len()!=24 || !text.iter().all(u8::is_ascii_uppercase){return Err("bad challenge literal".into());}
        let mut rotation=2;let mut reply=Vec::new();
        for &letter in text {let value=letter-b'A';reply.push(b'A'+(value+rotation)%26);rotation=(rotation+3+value)%26;}
        Ok(reply)
    }

'''+t[point:]
path.write_text(t)
print('Prepared drain-only mixed-protocol load',path)
