"""Add clocks only to generated diagnostic examples, never the counted source."""


def instrument(text):
    field='    world_cpu: f64,\n'
    assert text.count(field)==1
    text=text.replace(field,field+''.join(f'    {name}_monotonic: f64,\n' for name in
        ['world_start','world_end','admission_start','admission_end']))
    for variable,clock in [('s','start'),('base','wall')]:
        if variable=='base' and 'fn storm_tick' not in text:continue
        for phase,following in [('world','if profiled {'),('admission',f'self.admission(&mut {variable});')]:
            anchor=f'        let {clock} = Instant::now();\n        let c = cpu(3);\n        {following}'
            assert text.count(anchor)==1,(variable,phase,'start')
            text=text.replace(anchor,f'        {variable}.{phase}_start_monotonic = cpu(1);\n'+anchor)
            anchor=f'        {variable}.{phase}_wall = {clock}.elapsed().as_secs_f64();'
            assert text.count(anchor)==1,(variable,phase,'end')
            text=text.replace(anchor,anchor+f'\n        {variable}.{phase}_end_monotonic = cpu(1);')
    return text
