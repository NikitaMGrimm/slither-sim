// Research preparation acceleration of the copied host admission predicates.
// Original admission references keep the original search. No physics is changed.
use crate::physics::{Bounds, collision_distance, segment_distance, swept_distance};

fn bounds_of(body: impl Iterator<Item=Point>) -> Bounds {
    let mut bounds=Bounds::between(Point::default(),Point::default());
    let mut first=true;
    for p in body {if first {bounds=Bounds::between(p,p);first=false;} else {bounds.include(p);}}
    bounds
}
fn regular(bounds:&Bounds)->bool {
    [bounds.min.x,bounds.min.y,bounds.max.x,bounds.max.y].iter().all(|v|v.is_finite() && v.abs()<=1e7)
}
fn warm_clear(world:&Simulator,bounds:&[Option<Bounds>],body:&[Point],clearance:f64,excluded:Option<usize>)->bool {
    if !(2..=430).contains(&body.len()) || !body.iter().all(|p|p.finite()) || !clearance.is_finite() || clearance<0.
        || body.iter().any(|p|p.distance(Point::default())>=world.radius()-crate::simulator::SPAWN_WALL_MARGIN) {return false;}
    let query=bounds_of(body.iter().copied());
    world.snakes().iter().enumerate().filter(|(i,s)|s.alive && Some(*i)!=excluded).all(|(i,s)| {
        let width=(collision_distance(s.segments,body.len())+40.).max(clearance);
        let bounds=bounds[i].expect("fresh living bounds");
        // Every tested line is inside its box. One full world unit dwarfs the
        // roundoff of <=1e7 operands, including geometry's subtraction/dot math.
        if regular(&query) && regular(&bounds) && !bounds.near(query,width+1.) {return true;}
        body.windows(2).all(|edge| {
            let (head,tail)=(edge[0],edge[1]);
            if segment_distance(s.head,head,tail)<=width {return false;}
            let mut a=s.head;
            s.body.iter().all(|&b| {let distance=swept_distance(head,tail,a,b);a=b;distance>width})
        })
    })
}
fn warm_margin(world:&Simulator,bounds:&[Option<Bounds>],body:&[Point],stop_at:f64,excluded:Option<usize>)->Option<f64> {
    if !(2..=430).contains(&body.len()) || body.iter().any(|p|!p.finite() || p.distance(Point::default())>=world.radius()-crate::simulator::SPAWN_WALL_MARGIN) {return None;}
    let query=bounds_of(body.iter().copied());
    let mut margin=f64::INFINITY;
    for (i,s) in world.snakes().iter().enumerate().filter(|(i,s)|s.alive && Some(*i)!=excluded) {
        let contact=collision_distance(s.segments,body.len())+40.;
        let bounds=bounds[i].expect("fresh living bounds");
        let gap=(query.min.x-bounds.max.x).max(bounds.min.x-query.max.x)
            .max(query.min.y-bounds.max.y).max(bounds.min.y-query.max.y).max(0.);
        // This lower bound cannot lower the accumulated minimum. Retain the
        // original snake/edge order and bounded-search return values elsewhere.
        if regular(&query) && regular(&bounds) && gap-contact-1.>margin {continue;}
        for edge in body.windows(2) {
            let (head,tail)=(edge[0],edge[1]);
            margin=margin.min(segment_distance(s.head,head,tail)-contact);
            if margin<=stop_at {return Some(margin);}
            let mut a=s.head;
            for &b in &s.body {
                margin=margin.min(swept_distance(head,tail,a,b)-contact);
                if margin<=stop_at {return Some(margin);}
                a=b;
            }
        }
    }
    Some(margin)
}
