"""Restart only the checkpoint-less interrupted 10k cohort, retaining its log."""
import json
from pathlib import Path
import shutil
import sys
from run import digest, emit, execute


def restart(root):
    root=Path(root).resolve()
    partial=root/'results/10000'
    assert partial.exists() and not (partial/'fixture/world.json').exists()
    assert not (partial/'warm-qualification.json').exists()
    interrupted=root/'results/10000-interrupted'
    assert not interrupted.exists()
    partial.rename(interrupted)
    receipt={'reason':'original controller and child disappeared; no terminal status or checkpoint',
             'partialWarmLogSha256':digest(interrupted/'warm.jsonl'),
             'masterPinsSha256':digest(root/'measurement-pins.json'),
             'runnerSha256':digest(root/'run.py'),'restartHelperSha256':digest(Path(__file__)),
             'restartedPopulation':10000,'completedPopulationsUnchanged':[1000,2500,5000]}
    (interrupted/'interruption.json').write_text(json.dumps(receipt,indent=2)+'\n')
    # Reuse the exact frozen controller and executable, with a fresh output root.
    temporary=root/'restart-execution';temporary.mkdir(exist_ok=False)
    (temporary/'target').symlink_to(root/'target',target_is_directory=True)
    (temporary/'measurement-pins.json').symlink_to(root/'measurement-pins.json')
    emit('restart-primary',**receipt)
    execute(temporary,[10000])
    shutil.move(str(temporary/'results/10000'),str(root/'results/10000'))
    emit('complete',restartedPopulation=10000)


if __name__=='__main__':restart(sys.argv[1])
