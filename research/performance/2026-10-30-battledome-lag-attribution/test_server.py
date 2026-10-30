"""Short offline HTTP-boundary checks, including execution with python -O."""
from pathlib import Path
import io
import json
import runpy
import sys
import tempfile
import unittest
from unittest.mock import patch


class ServerBoundaryTests(unittest.TestCase):
    def exercise(self,action,label,mode="off"):
        with tempfile.TemporaryDirectory() as temporary:
            root=Path(temporary)/"study";root.mkdir()
            source=Path(__file__).with_name("server.py")
            with patch.object(sys,"argv",[str(source),str(root),"0"]),patch("http.server.HTTPServer"):
                namespace=runpy.run_path(str(source))
            handler=object.__new__(namespace["Handler"])
            payload=json.dumps({"action":action,"label":label,"mode":mode}).encode()
            handler.headers={"Content-Length":str(len(payload))}
            handler.rfile=io.BytesIO(payload);handler.wfile=io.BytesIO()
            responses=[];handler.send_response=responses.append;handler.end_headers=lambda:None
            with patch("subprocess.Popen",side_effect=AssertionError("Unexpected process launch")):
                handler.do_POST()
            self.assertEqual(responses,[500])
            self.assertIn(b"ValueError",handler.wfile.getvalue())
            self.assertEqual(list(root.iterdir()),[root/"session"])
            self.assertEqual(list((root/"session").iterdir()),[])

    def test_invalid_labels_cannot_reach_filesystem(self):
        for action in ["mode","checkpoint"]:
            for label in ["../escape","/absolute",r"..\escape","a/b","",None]:
                with self.subTest(action=action,label=label):self.exercise(action,label)

    def test_invalid_mode_is_explicitly_rejected(self):
        self.exercise("mode","07-primary","unknown")


if __name__=="__main__":unittest.main()
