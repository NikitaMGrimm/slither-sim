"""Drain real protocol-13 output; drive it with an equal stock-world A4 twin."""
def legacy_fixture(text):
    edits=[
        ('    let dt=sim.dt();', '''    let mut driver=RawSimulator::new(seed,SimulationConfig {
        agents:count,world:crate::Config::battledome(),..Default::default()
    },count).unwrap();
    let dt=sim.dt();'''),
        ('            if !sim.grow_for_waiters(queue.len(),tick>=protected)',
         '            let grown=sim.grow_for_waiters(queue.len(),tick>=protected);assert_eq!(grown,driver.grow_for_waiters(queue.len(),tick>=protected));\n            if !grown'),
        ('                if sim.connect_waited(slot,PlayerAppearance {nickname:format!("Local{slot}").into_bytes(),skin:(slot%9) as u8,..Default::default()},(tick-start) as f64*dt).unwrap() {',
         '''                let profile=PlayerAppearance {nickname:format!("Local{slot}").into_bytes(),skin:(slot%9) as u8,..Default::default()};
                let admitted=sim.connect_waited(slot,profile.clone(),(tick-start) as f64*dt).unwrap();
                assert_eq!(admitted,driver.connect_waited(slot,profile,(tick-start) as f64*dt).unwrap());
                if admitted {'''),
        ('sim.physical_food(slot);', 'sim.protocol13(slot);sim.physical_food(slot);driver.physical_food(slot);'),
        ('sim.receive_at(slot,&packet,now).unwrap();','sim.receive_at(slot,&packet,now).unwrap();driver.receive_at(slot,&packet,now).unwrap();'),
        ('        sim.advance_tick_with(0,&mut |_|Ok(())).unwrap();',
         '        sim.advance_tick_with(0,&mut |_|Ok(())).unwrap();driver.advance_tick_with(0,&mut |_|Ok(())).unwrap();\n        assert_eq!(sim.backend.world.snakes(),driver.backend.world.snakes());'),
        ('                states[slot].receive_frame(&frame,at).unwrap();black_box(frame);',
         '''                black_box(frame);
            }
            for(at,frame) in driver.take(slot).unwrap() {
                assert!(at<=now+1e-9);states[slot].receive_frame(&frame,at).unwrap();'''),
        ('sim.disconnect(slot).unwrap();','sim.disconnect(slot).unwrap();driver.disconnect(slot).unwrap();'),
        ('    let census:Vec<_>=', '''    assert_eq!(serde_json::to_vec(&sim.backend.world.checkpoint().unwrap()).unwrap(),serde_json::to_vec(&driver.backend.world.checkpoint().unwrap()).unwrap());
    assert_eq!(format!("{:?}",sim.backend.spawn_random),format!("{:?}",driver.backend.spawn_random));
    let census:Vec<_>=''')]
    for before,after in edits:
        assert text.count(before)==1,before
        text=text.replace(before,after)
    return text
