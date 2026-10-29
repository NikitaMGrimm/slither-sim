"""Study-only high boost output; shared A4 steering and policy stay untouched."""
from pathlib import Path
import runpy,json,hashlib,tarfile
r=Path(__file__).resolve().parent
runpy.run_path(str(r/'prepare262.py'))
p=r/'server/source/examples/aws_clients.rs';s=p.read_text()
old='if let Some(action)=action{for packet in action_packets(action,s.boost)?'
assert s.count(old)==1
s=s.replace(old,'if let Some(action)=action{let action=high_duty(action,frame.rules.as_ref());for packet in action_packets(action,s.boost)?')
s+='''
// Research treatment only. Steering remains the exact shared A4 result.
fn high_duty(mut action:Action,rules:Option<&slither_sim::WorldRules>)->Action{
 if rules.is_some_and(|rules|rules.free_boost_at_floor){action.boost=true;}
 action
}
#[test]
fn high_duty_preserves_heading_and_rule_gate(){
 let allowed=slither_sim::WorldRules{free_boost_at_floor:true,..Default::default()};
 let ordinary=slither_sim::WorldRules::default();
 for heading in [-20.0,-std::f64::consts::PI,-0.0,0.0,1.0,std::f64::consts::TAU,20.0]{
  for boost in [false,true]{
   let action=Action{heading,boost};let forced=high_duty(action,Some(&allowed));
   assert_eq!(forced.heading.to_bits(),action.heading.to_bits());assert!(forced.boost);
   assert_eq!(high_duty(action,None),action);assert_eq!(high_duty(action,Some(&ordinary)),action);
  }
 }
}
'''
p.write_text(s,encoding='utf-8',newline='\n')
pins_path=r/'prepared.json';pins=json.loads(pins_path.read_text());pins['clientSourceSha256']=hashlib.file_digest(p.open('rb'),'sha256').hexdigest();pins['boostTreatment']='Force boost when received freeBoostAtFloor=true; preserve A4 heading bits';pins_path.write_text(json.dumps(pins,indent=2)+'\n')
with tarfile.open(r/'main.tar') as archive:
 policy={member.name:hashlib.sha256(archive.extractfile(member).read()).hexdigest() for member in archive if member.isfile() and (member.name.startswith('src/bot/') or member.name=='src/client/forager.rs')}
assert all(hashlib.file_digest((r/'server/source'/name).open('rb'),'sha256').hexdigest()==digest for name,digest in policy.items())
pins['sharedPolicyUnchanged']=True;pins['sharedPolicySha256']=policy;pins_path.write_text(json.dumps(pins,indent=2)+'\n')
print('CONSTANT-BOOST HARNESS PREPARED; shared policy source unchanged')
