//! Research-only child of client::simulation: deterministic hosted admission/A4.
use super::{PlayerAppearance, RawSimulator, SimulationConfig};
use crate::client::{Bot, ClientState, ClientStatus, ForagerBot, protocol::action_packets};
use std::{collections::VecDeque, hint::black_box, io::{self, Write}};

pub fn run(count: usize, seed: u32, trace: bool) {
    assert!([64,80].contains(&count));
    let mut sim=RawSimulator::new(seed,SimulationConfig {
        agents:count,world:crate::Config::battledome(),..Default::default()
    },count).unwrap();
    let dt=sim.dt();let ticks=(60./dt).ceil() as usize;
    let mut queue:VecDeque<_>=(0..count).map(|slot|(slot,0usize)).collect();
    let mut next_spawn=0;let mut last_spawn:Option<usize>=None;
    let mut states:Vec<_>=(0..count).map(|_|ClientState::new()).collect();
    let mut bots:Vec<_>=(0..count).map(|_|ForagerBot::new()).collect();
    let mut boosts=vec![None;count];let mut open=vec![false;count];
    let mut born=vec![None;count];let mut followed=vec![false;count];
    let mut ever=vec![false;count];let mut all_waits=Vec::new();let mut rejoin_waits=Vec::new();
    let(mut admissions,mut refusals,mut deaths,mut early,mut followups)=(0,0,0,0,0);
    let(mut frames,mut bytes,mut playing_sum,mut waiting_sum)=(0u64,0u64,0u64,0u64);
    let mut out=io::BufWriter::new(io::stdout().lock());
    let mut record=|tag:u64,payload:&[u8]|{
        if trace {out.write_all(&tag.to_le_bytes()).unwrap();out.write_all(&(payload.len() as u64).to_le_bytes()).unwrap();out.write_all(payload).unwrap();}
    };
    if trace {record(0,&serde_json::to_vec(&sim.backend.world.checkpoint().unwrap()).unwrap());}
    for tick in 0..ticks {
        if tick>=next_spawn && !queue.is_empty() {
            let protected=last_spawn.map_or(0,|t|t+(crate::spawn::DEPARTURE_SECONDS/dt).ceil() as usize);
            if !sim.grow_for_waiters(queue.len(),tick>=protected) {next_spawn=protected;}
            else {
                let(slot,start)=queue.pop_front().unwrap();
                if sim.connect_waited(slot,PlayerAppearance {nickname:format!("Local{slot}").into_bytes(),skin:(slot%9) as u8,..Default::default()},(tick-start) as f64*dt).unwrap() {
                    let wait=(tick-start) as f64*dt;
                    all_waits.push(wait);if ever[slot] {rejoin_waits.push(wait);}
                    ever[slot]=true;admissions+=1;born[slot]=Some(tick);followed[slot]=false;
                    sim.physical_food(slot);states[slot]=ClientState::new();states[slot].new_connection(sim.backend.time()).unwrap();bots[slot].reset();boosts[slot]=None;open[slot]=true;
                    last_spawn=Some(tick);next_spawn=tick+1;
                    if trace {record(1,&serde_json::to_vec(&(tick,slot,wait,sim.backend.world.snakes()[slot].clone(),format!("{:?}",sim.backend.spawn_random))).unwrap());}
                } else {
                    refusals+=1;queue.push_front((slot,start));next_spawn=tick+(crate::spawn::RETRY_SECONDS/dt).ceil() as usize;
                }
            }
        }
        let now=sim.backend.time();
        if tick.is_multiple_of(6) {
            for slot in 0..count {
                if open[slot] && states[slot].status()==ClientStatus::Playing {
                    let frame=states[slot].frame(now).unwrap();
                    if let Some(action)=bots[slot].decide(&frame) {
                        for packet in action_packets(action,boosts[slot]).unwrap() {
                            states[slot].record_sent(&packet,now).unwrap();sim.receive_at(slot,&packet,now).unwrap();
                        }
                        boosts[slot]=Some(action.boost);
                    }
                }
            }
        }
        sim.advance_tick_with(0,&mut |_|Ok(())).unwrap();
        let now=sim.backend.time();
        for slot in 0..count {
            for(at,frame) in sim.take(slot).unwrap() {
                assert!(at<=now+1e-9);frames+=1;bytes+=frame.len() as u64;
                if trace {
                    let mut header=Vec::with_capacity(24);header.extend_from_slice(&(tick as u64).to_le_bytes());header.extend_from_slice(&(slot as u64).to_le_bytes());header.extend_from_slice(&at.to_bits().to_le_bytes());
                    record(2,&header);record(3,&frame);
                }
                states[slot].receive_frame(&frame,at).unwrap();black_box(frame);
            }
            if let Some(start)=born[slot] {
                let age=(tick+1-start) as f64*dt;
                if !sim.backend.world.snakes()[slot].alive {
                    deaths+=1;if age<=3. {early+=1;}
                    if trace {record(4,&serde_json::to_vec(&(tick,slot,age)).unwrap());}
                    sim.disconnect(slot).unwrap();born[slot]=None;open[slot]=false;queue.push_back((slot,tick+1));
                } else if age>=3. && !followed[slot] {followed[slot]=true;followups+=1;}
            }
        }
        playing_sum+=sim.backend.world.snakes().iter().filter(|s|s.alive).count() as u64;
        waiting_sum+=queue.len() as u64;
    }
    let quantiles=|values:&[f64]| {
        if values.is_empty(){return serde_json::Value::Null;}
        let mut v=values.to_vec();v.sort_by(f64::total_cmp);
        serde_json::json!({"n":v.len(),"p50":v[(v.len()-1)/2],"p95":v[(0.95*v.len() as f64).ceil() as usize-1],"p99":v[(0.99*v.len() as f64).ceil() as usize-1],"max":v[v.len()-1]})
    };
    let census:Vec<_>=queue.iter().map(|&(_,start)|(ticks-start) as f64*dt).collect();
    let summary=serde_json::json!({"seats":count,"seed":seed,"ticks":ticks,"admissions":admissions,"refusals":refusals,"deaths":deaths,"earlyDeaths3s":early,"complete3sFollowups":followups,"lateUnfollowed":born.iter().enumerate().filter(|(i,v)|v.is_some()&&!followed[*i]).count(),"everAdmitted":ever.iter().filter(|&&v|v).count(),"frames":frames,"bytes":bytes,"playingSum":playing_sum,"waitingSum":waiting_sum,"playingFinal":sim.backend.world.snakes().iter().filter(|s|s.alive).count(),"waitAllSeconds":quantiles(&all_waits),"waitRejoinSeconds":quantiles(&rejoin_waits),"censoredWaitSeconds":quantiles(&census),"spawnRandom":format!("{:?}",sim.backend.spawn_random)});
    assert!(ever.iter().all(|&v|v) && followups>=count && frames>0 && admissions>count);
    if trace {record(5,&serde_json::to_vec(&sim.backend.world.checkpoint().unwrap()).unwrap());}
    record(6,&serde_json::to_vec(&summary).unwrap());drop(record);out.flush().unwrap();eprintln!("{summary}");
}
