"""Add CPU accounting and optional socket niceness to the frozen #211 worker."""
from pathlib import Path


def patch(path):
    path = Path(path)
    text = path.read_text(encoding='utf-8')
    old = "return {'cpuSeconds': (int(stat[11]) + int(stat[12])) / HZ,"
    assert text.count(old) == 1
    text = text.replace(old, "return {'userSeconds': int(stat[11]) / HZ,\n"
                        "                'systemSeconds': int(stat[12]) / HZ,\n"
                        "                'nice': int(stat[16]),\n"
                        "                'cpuSeconds': (int(stat[11]) + int(stat[12])) / HZ,")
    old = '        pinned = False\n'
    assert text.count(old) == 1
    text = text.replace(old, old + '        study_niced = False\n')
    old = '            now = time.time()\n'
    assert text.count(old) == 1
    text = text.replace(old, old + '''            if args.role == 'host' and not study_niced and os.environ.get('STUDY_SOCKET_NICE'):
                for tid in Path(f'/proc/{pids[0]}/task').glob('*'):
                    try:
                        if (tid/'comm').read_text().strip() == 'slither-sockets':
                            value = int(os.environ['STUDY_SOCKET_NICE'])
                            os.setpriority(os.PRIO_PROCESS, int(tid.name), value)
                            assert os.getpriority(os.PRIO_PROCESS, int(tid.name)) == value
                            (out/'nice-applied.json').write_text(json.dumps({'unix': now, 'nice': value}))
                            study_niced = True
                    except (FileNotFoundError, ProcessLookupError):
                        pass
''')
    path.write_text(text, encoding='utf-8', newline='\n')
