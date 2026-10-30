"""Add clocks and non-authoritative event audits only to diagnostic examples."""

AUDIT = r'''
// Reconstruction uses post-step circle fields: food/prey gains change fam,
// not head, heading, speed, segments or alive. Ambient births follow pickup.
fn pickup_guard_after_step(world: &slither_sim::Simulator, before: (usize, f64)) -> Value {
    let picked = world.pickup_events().unwrap().iter()
        .filter(|event| matches!(event.item, slither_sim::PickupItem::Food(_))).count();
    let ambient = world.food_spawn_events().unwrap().iter().filter(|event| event.source.is_none()).count();
    let food = (world.food().len() + picked).checked_sub(ambient).unwrap();
    let nonambient = world.food_spawn_events().unwrap().iter().filter(|event| event.source.is_some()).count();
    assert_eq!(before.1.to_bits(),world.radius().to_bits(),"Audit requires the frozen rows' unchanged radius");
    assert_eq!(food,before.0+nonambient,"Independent food conservation at pickup entry");
    let mut lo = [i64::MAX; 2];let mut hi = [i64::MIN; 2];
    let mut area = 0usize;let mut regular = true;let mut circles = 0usize;
    if food>0 {
        for snake in world.snakes().iter().filter(|s|s.alive) {
            let (center,radius) = slither_sim::physics::pickup_circle(
                snake.head,snake.heading,snake.speed,snake.segments);
            regular &= radius>=0.0 && center.x.abs()+radius<=1048576.0 && center.y.abs()+radius<=1048576.0;
            let lower=[((center.x-radius-1e-9)/128.0).floor() as i64,((center.y-radius-1e-9)/128.0).floor() as i64];
            let upper=[((center.x+radius+1e-9)/128.0).floor() as i64,((center.y+radius+1e-9)/128.0).floor() as i64];
            for axis in 0..2 {lo[axis]=lo[axis].min(lower[axis]);hi[axis]=hi[axis].max(upper[axis]);}
            area=area.saturating_add(((upper[0]-lower[0]+1)*(upper[1]-lower[1]+1)) as usize);
            circles+=1;
        }
    }
    let cells=if circles==0 {0} else {((hi[0]-lo[0]+1)*(hi[1]-lo[1]+1)) as usize};
    let food_limit=food.saturating_mul(4).max(4096);
    let a=cells>1048576;let b=cells>food_limit;let c=area>1048576;
    let dense=world.config().food_index && circles>0 && regular && !a && !b && !c && circles<(u32::MAX as usize);
    let indexed=!dense && world.config().food_index && circles.saturating_mul(food)>=32000;
    json!({"boxCells":cells,"foodLen":food,"coverArea":area,"circles":circles,
        "foodGuardLimit":food_limit,"fixedCellLimit":1048576,"regularGeometry":regular,
        "declineFixedCells":a,"declineFoodLength":b,"declineCoverArea":c,
        "dense":dense,"indexed":indexed,"path":if dense {"dense"} else if indexed {"hashed indexed"} else {"linear"},
        "foodPickups":picked,"ambientBirths":ambient,"finalFoodLen":world.food().len(),
        "foodBeforeWorld":before.0,"nonAmbientBirths":nonambient,"foodConservationVerified":true})
}
'''


def instrument(text):
    field='    world_cpu: f64,\n'
    assert text.count(field)==1
    text=text.replace(field,field+''.join(f'    {name}_monotonic: f64,\n' for name in
        ['world_start','world_end','admission_start','admission_end'])+'    pickup_guard: Option<Value>,\n')
    anchor='        let world = Simulator::from_checkpoint('
    assert text.count(anchor)==1
    text=text.replace(anchor,'        let mut world = Simulator::from_checkpoint(')
    anchor='        let meta = serde_json::from_slice(&fs::read(root.join("meta.json")).unwrap()).unwrap();'
    assert text.count(anchor)==1
    text=text.replace(anchor,anchor+'\n        world.record_events(slither_sim::EventRecording { pickups: true, food_births: true });')
    for variable,clock in [('s','start'),('base','wall')]:
        if variable=='base' and 'fn storm_tick' not in text:continue
        for phase,following in [('world','if profiled {'),('admission',f'self.admission(&mut {variable});')]:
            anchor=f'        let {clock} = Instant::now();\n        let c = cpu(3);\n        {following}'
            assert text.count(anchor)==1,(variable,phase,'start')
            text=text.replace(anchor,f'        {variable}.{phase}_start_monotonic = cpu(1);\n'+anchor)
            if phase=='world':
                anchor=f'        {variable}.world_start_monotonic = cpu(1);'
                text=text.replace(anchor,'        let pickup_audit_before = (self.world.food().len(), self.world.radius());\n'+anchor)
            anchor=f'        {variable}.{phase}_wall = {clock}.elapsed().as_secs_f64();'
            assert text.count(anchor)==1,(variable,phase,'end')
            text=text.replace(anchor,anchor+f'\n        {variable}.{phase}_end_monotonic = cpu(1);')
            if phase=='world':
                anchor=f'        {variable}.world_end_monotonic = cpu(1);'
                text=text.replace(anchor,anchor+f'\n        {variable}.pickup_guard = Some(pickup_guard_after_step(&self.world, pickup_audit_before));')
    return text+AUDIT
