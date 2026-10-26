"""Offline tests for observed layouts and explicitly scrubbed capture witnesses."""
import builtins
import base64
import importlib.util
import json
from pathlib import Path
import unittest
from unittest.mock import patch

import protocol_catalog as protocol

ROOT=Path(__file__).resolve().parents[2]


class CatalogTest(unittest.TestCase):
    def test_header_and_version_labels_are_literal(self):
        for version in (13,19):
            path=ROOT/'tests/fixtures'/f'protocol-catalog-{version}.json'
            fixture=json.loads(path.read_text())
            self.assertEqual(fixture['wireProtocol'],version)
            self.assertIn('Derived captured',fixture['transformation'])
            setups=[];codes=set();roles=set()
            for row in fixture['records']:
                frame=base64.b64decode(row['frameBase64'],validate=True)
                actual=[protocol.compact(protocol.decode(p,version,row['own'])) for _,p in protocol.split(frame)]
                self.assertEqual(actual,row['expected'],(path.name,row['record']))
                for _,p in protocol.split(frame):
                    codes.add(chr(p[0]))
                    if p[0]==ord('a'):setups.append(p[23])
                    if p[0]==ord('s') and len(p)>7:
                        d=protocol.decode(p,version)
                        roles.add('self' if row['own'] is None or row['own']==d['id'] else 'peer')
                self.assertRegex(row['rawFrameSha256'],r'^[0-9a-f]{64}$')
            self.assertEqual(set(setups),{version})
            self.assertTrue(set('asvMlfbprjyzS')<=codes,codes)
            self.assertEqual(roles,{'self','peer'})
            if version==19:self.assertTrue(set('Vd7<=')<=codes)
            else:self.assertIn('c',codes)

    def test_names_and_identity_are_scrubbed_at_all_known_fields(self):
        for version in (13,19):
            fixture=json.loads((ROOT/'tests/fixtures'/f'protocol-catalog-{version}.json').read_text())
            kinds=set()
            for row in fixture['records']:
                frame=base64.b64decode(row['frameBase64'],validate=True);expected=[]
                for offset,p in protocol.split(frame):
                    cleaned,ranges=protocol.scrub(p)
                    self.assertEqual(cleaned,p)
                    expected.extend([[offset+a,offset+b,kind]for a,b,kind in ranges])
                self.assertEqual(row['redactions'],expected)
                for start,end,kind in expected:
                    kinds.add(kind)
                    fill=b'x' if kind in ('name','record-text') else b'\0'
                    self.assertEqual(frame[start:end],fill*(end-start))
            self.assertIn('name',kinds)
            self.assertIn('server-number',kinds)

    def test_independent_checked_anchor_and_motion_examples(self):
        # Handwritten synthetic coordinates, not generated decoder expectations.
        old=protocol.decode(bytes.fromhex('470123817f'),13)
        self.assertEqual((old['id'],old['delta']),(291,[1,-1]))
        absolute=protocol.decode(bytes.fromhex('6701237f267f50'),13)
        self.assertEqual(absolute['position'],[32550,32592])
        new=protocol.decode(bytes.fromhex('471234'),19,own=42)
        self.assertEqual((new['id'],new['angle']),(42,4660))
        with self.assertRaises(ValueError):protocol.decode(bytes.fromhex('471234'),13)
        speed=protocol.decode(bytes.fromhex('3307'),19,own=42)
        self.assertEqual(speed['speedByte'],7)
        with self.assertRaises(ValueError):protocol.decode(bytes.fromhex('3307'),13)

    def test_same_food_bytes_have_different_layouts(self):
        p=bytes.fromhex('460500ff01020a')
        self.assertEqual(protocol.decode(p,13)['foods'],[[5,255,258,10]])
        new=protocol.decode(p,19)
        self.assertEqual(new['sector'],[5,0])
        self.assertEqual(new['foods'],[[255,1,2,10]])

    def test_unknown_and_truncated_data_fail_visibly(self):
        for frame in (b'',b'\0\0',bytes.fromhex('00000005aa')):
            with self.assertRaises(ValueError):protocol.split(frame)
        for p in (b'?',b'g',b'j\0',b's\0',b'F\0',b'M\0'):
            with self.assertRaises((ValueError,IndexError)):protocol.decode(p,13)
        with self.assertRaises(ValueError):protocol.decode(b'p',12)

    def test_import_needs_no_network_modules(self):
        original=builtins.__import__
        def guarded(name,*args,**kwargs):
            if name.split('.')[0] in ('socket','http','urllib','requests','websockets'):
                raise AssertionError('network import '+name)
            return original(name,*args,**kwargs)
        spec=importlib.util.spec_from_file_location('offline_catalog',protocol.__file__)
        with patch('builtins.__import__',guarded):
            module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module)
            self.assertEqual(module.decode(b'p',13)['code'],'p')


if __name__=='__main__':unittest.main()
