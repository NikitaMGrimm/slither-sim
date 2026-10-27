"""Legacy emission scopes over the frozen #210 profiler; Linux x86-64 only."""
import sys,tarfile,re
from pathlib import Path
if not __debug__:raise SystemExit('run without -O')
# Same ARCHIVE OUT --control BINARY interface; optional --fine and --fixture FILE.
args=sys.argv[1:];fixture=None
if '--fixture' in args:
 i=args.index('--fixture');fixture=Path(args[i+1]);del args[i:i+2]
archive=Path(args[0])
with tarfile.open(archive) as t:
 prefix='research/performance/2026-10-27-host-food-ranges/'
 driver=t.extractfile(prefix+'profile.py').read().decode()
 counter=t.extractfile(prefix+'profile.rs').read().decode()
counter=counter.replace('const N:usize=16;','const N:usize=10;')
counter=re.sub(r'const NAMES:\[&str;N\]=\[.*?\];','const NAMES:[&str;N]=["emit","conversion","grouping","arrivals","frame_loop","frame_encode","anchor","food_convert","fullmap","appearance"];',counter)
start=driver.index("b='src/client/simulation/broadcast.rs'");end=driver.index('\nenv=dict(os.environ',start)
hooks='c=\'src/client/simulation/connection.rs\';e=\'src/client/encode13.rs\'\nedit(c,\'        if self.slots[slot].connection.output_fault.is_some() {\',\'        let _study_emit=crate::recipient_profile::Scope::new(0);\\n        if self.slots[slot].connection.output_fault.is_some() {\')\nscope(e,\'    pub(crate) fn packets(&mut self, packets: &[Vec<u8>]) -> Result<Vec<Vec<u8>>, String> {\',1)\nedit(c,\'        let groups = if self.slots[slot].connection.legacy.is_some() {\',\'        let study_groups=crate::recipient_profile::Scope::new(2);\\n        let groups = if self.slots[slot].connection.legacy.is_some() {\')\nedit(c,\'        let latency = self.player_latency.for_slot(slot, &self.config.latency);\\n        let connection = &mut self.slots[slot].connection;\\n        if !remote\', \'        drop(study_groups);\\n        let latency = self.player_latency.for_slot(slot, &self.config.latency);\\n        let connection = &mut self.slots[slot].connection;\\n        if !remote\')\nedit(c,\'        let arrivals = physical\',\'        let study_arrivals=crate::recipient_profile::Scope::new(3);\\n        let arrivals = physical\')\nedit(c,\'        for (index, group) in groups.into_iter().enumerate() {\',\'        drop(study_arrivals);\\n        let _study_loop=crate::recipient_profile::Scope::new(4);\\n        for (index, group) in groups.into_iter().enumerate() {\')\nscope(\'src/client/encode.rs\',\'pub(crate) fn frame(elapsed_ms: u16, packets: &[Vec<u8>]) -> Result<Vec<u8>, String> {\',5)\nscope(e,\'    fn anchor(&mut self, p: &[u8]) -> Result<Vec<u8>, String> {\',6)\nedit(e,"                b\'F\' => {", "                b\'F\' => {\\n                    let _study_food=crate::recipient_profile::Scope::new(7);")\nedit(e,"                b\'f\' | b\'b\' => {", "                b\'f\' | b\'b\' => {\\n                    let _study_food=crate::recipient_profile::Scope::new(7);")\nedit(e,"                b\'c\' | b\'<\' => {", "                b\'c\' | b\'<\' => {\\n                    let _study_food=crate::recipient_profile::Scope::new(7);")\nscope(e,\'    fn packet(&mut self, p: &[u8]) -> Result<Vec<u8>, String> {\',8)\nscope(e,\'    fn appearance(&mut self, p: &[u8]) -> Result<Vec<u8>, String> {\',9)\nif not a.fine:\n import re\n for name in [c,e,\'src/client/encode.rs\']:\n  p=source/name;p.write_text(\'\\n\'.join(line for line in p.read_text().splitlines() if not re.search(r\'Scope::new\\([5679]\\)\',line))+\'\\n\')\n'
driver=driver[:start]+hooks+driver[end:]
driver=driver.replace("shutil.copy2(folder/'profile.rs',source/'src/recipient_profile.rs')","(source/'src/recipient_profile.rs').write_text("+repr(counter)+")")
driver=driver.replace("p.add_argument('--control',type=Path,required=True)","p.add_argument('--control',type=Path,required=True);p.add_argument('--fine',action='store_true')").replace('range(16)','range(10)')
if fixture:
 driver=driver.replace("fixture=(study/'protocol_fixture.rs').read_text()","fixture=Path("+repr(str(fixture.resolve()))+").read_text()")
 driver=driver.replace("['official500','official500-half','official500-legacy']","['official500','official500-half','official500-legacy','official500-fullmap']")
sys.argv=[sys.argv[0],*args]
exec(compile(driver,str(archive)+'-diagnostic','exec'))
