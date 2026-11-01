# Native egui dependency licences

`egui`, `egui-wgpu` and `egui-winit` are pinned to `=0.37.0`, with defaults
disabled, and offer MIT OR Apache-2.0. Their native dependency tree shares the
existing wgpu 30.0.1 and winit 0.30.13. They are excluded from the browser and
simulator dependency trees. No font files are bundled: the native theme reads
the installed fonts already used by the client.

The 34 added lockfile packages were checked against their published manifests;
each offers a permissive licence choice. `self_cell` uses its Apache-2.0 option,
rather than its GPL alternative. `unicode-properties` 0.1.5 requires
`(MIT OR Apache-2.0) AND Unicode-3.0`; its complete additional copyright and
permission notice is retained in [UNICODE-LICENSE.txt](../third-party/UNICODE-LICENSE.txt).
Keep that notice with distributed copies or their documentation.
