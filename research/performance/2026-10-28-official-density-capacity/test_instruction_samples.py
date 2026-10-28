"""Guard ELF translation where text virtual and file offsets differ."""
import importlib.util
from pathlib import Path
import struct
import tempfile
import unittest

spec=importlib.util.spec_from_file_location('instruction_samples',Path(__file__).with_name('instruction_samples.py'))
samples=importlib.util.module_from_spec(spec);spec.loader.exec_module(samples)

class ElfTranslation(unittest.TestCase):
    def test_executable_load_offset_is_not_assumed_to_equal_virtual_address(self):
        header=bytearray(64);header[:6]=b'\x7fELF\x02\x01'
        struct.pack_into('Q',header,32,64);struct.pack_into('HH',header,54,56,1)
        load=struct.pack('IIQQQQQQ',1,5,0x3000,0x5000,0,0x1000,0x1000,0x1000)
        with tempfile.TemporaryDirectory() as temp:
            binary=Path(temp)/'synthetic.elf';binary.write_bytes(header+load)
            self.assertEqual(samples.elf_relative(binary,0x3000,0x80),0x5080)
            with self.assertRaises(ValueError):samples.elf_relative(binary,0x9000,0)

if __name__=='__main__':unittest.main()
