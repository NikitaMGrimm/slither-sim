// Artifact hashes alone cannot establish that a stale build used this source tree.
export function assertBuildBinding(built, product) {
  if (built.tree !== product.tree || built.core !== product.core) throw new Error('Replay artifacts are stale; stage the candidate and rebuild explicitly');
}
