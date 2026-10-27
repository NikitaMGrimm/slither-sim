"""Emission brackets and input census on an immutable Linux x86-64 runtime."""
from pathlib import Path
import sys,tarfile,re,json
if not __debug__:raise SystemExit('run without -O')
archive=Path(sys.argv[1])
with tarfile.open(archive) as t:
    prefix='research/performance/2026-10-27-host-food-ranges/'
    driver=t.extractfile(prefix+'profile.py').read().decode()
    counter=t.extractfile(prefix+'profile.rs').read().decode()
names=['emit','conversion','grouping','arrivals','frame_loop','anchor','food','appearance','frame_owned','empty','pass_count','pass_bytes','anchor_count','food_count','own_motion_count','other_count','frame_room','frame_no_room']
counter=counter.replace('const N:usize=16;','const N:usize=18;')
counter=re.sub(r'const NAMES:\[&str;N\]=\[.*?\];','const NAMES:[&str;N]='+json.dumps(names,separators=(',',':'))+';',counter)
counter+='\npub(crate) fn event(i:usize,n:usize){DATA.with(|d|{let mut d=d.borrow_mut();if d.active{d.calls[i]+=n as u64;}});}\n'
start=driver.index("b='src/client/simulation/broadcast.rs'");end=driver.index('\nenv=dict(os.environ',start)
hooks='''
c='src/client/simulation/connection.rs';e='src/client/encode13.rs'
edit(c,'        if self.slots[slot].connection.output_fault.is_some() {','        let _study_emit=crate::recipient_profile::Scope::new(0);\\n        if self.slots[slot].connection.output_fault.is_some() {')
edit(e,'        let mut out = Vec::with_capacity(packets.len());','        let _study_conversion=crate::recipient_profile::Scope::new(1);\\n        let mut out = Vec::with_capacity(packets.len());')
edit(c,'        let (groups, physical) = if let Some(legacy) = &mut self.slots[slot].connection.legacy {','        let study_groups=crate::recipient_profile::Scope::new(2);\\n        let (groups, physical) = if let Some(legacy) = &mut self.slots[slot].connection.legacy {')
edit(c,'        let latency = self.player_latency.for_slot(slot, &self.config.latency);\\n        let connection = &mut self.slots[slot].connection;\\n        if !remote','        drop(study_groups);\\n        let latency = self.player_latency.for_slot(slot, &self.config.latency);\\n        let connection = &mut self.slots[slot].connection;\\n        if !remote')
edit(c,'        let arrivals = physical','        let study_arrivals=crate::recipient_profile::Scope::new(3);\\n        let arrivals = physical')
edit(c,'        for (index, group) in groups.enumerate() {','        drop(study_arrivals);\\n        let _study_loop=crate::recipient_profile::Scope::new(4);\\n        for (index, group) in groups.enumerate() {')
scope(e,'    fn anchor(&mut self, p: &[u8]) -> Result<Vec<u8>, String> {',5)
scope(e,'    fn appearance(&mut self, p: &[u8]) -> Result<Vec<u8>, String> {',7)
for line in ["                b'F' => {", "                b'f' | b'b' => {", "                b'c' | b'<' => {"]:
 edit(e,line,line+'\\n                    let _study_food=crate::recipient_profile::Scope::new(6);')
edit(e,'        for p in packets {',\'''        for p in packets {
            let i=match p[0]{b'g'|b'G'|b'n'|b'N'|b'='|b'+' =>12,b'F'|b'f'|b'b'|b'c'|b'<' =>13,b'd'|b'7'=>14,b'e'|b'E'|b'3'|b'4'|b'5' if p.len()<=3=>14,b's'|b'a'=>15,_=>10};
            crate::recipient_profile::event(i,1);
            if i==10{crate::recipient_profile::event(11,p.len());}
\''')
edit('src/client/encode.rs','pub(crate) fn frame_owned(elapsed_ms: u16, mut packet: Vec<u8>) -> Result<Vec<u8>, String> {','pub(crate) fn frame_owned(elapsed_ms: u16, mut packet: Vec<u8>) -> Result<Vec<u8>, String> {\\n    let _study_frame=crate::recipient_profile::Scope::new(8);\\n    let empty=crate::recipient_profile::Scope::new(9);drop(empty);\\n    crate::recipient_profile::event(if packet.capacity()-packet.len()>=2{16}else{17},1);')
'''
driver=driver[:start]+hooks+driver[end:]
driver=driver.replace("shutil.copy2(folder/'profile.rs',source/'src/recipient_profile.rs')","(source/'src/recipient_profile.rs').write_text("+repr(counter)+")").replace('range(16)','range(18)')
exec(compile(driver,str(archive)+'-emission','exec'))
