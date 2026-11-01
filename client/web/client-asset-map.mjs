import { copyAuthoredSourceRecipes, paintAuthoredCanvas } from './authored-source-recipes.mjs';

/** Default authored artwork from Rust plans, or an explicit private source factory. */
export async function createClientAssetMap({ realm = globalThis, sourceFactory, random, sourceRecipes } = {}) {
  if (sourceFactory !== undefined) {
    if (typeof sourceFactory !== 'function') throw new TypeError('Comparison source factory required');
    const sources = await sourceFactory({ realm, random });
    if (!Array.isArray(sources?.palettes) || sources.palettes.length !== 3
      || !sources.adapter || !sources.hostAssets) {
      throw new TypeError('Complete comparison source bundle required');
    }
    return sources;
  }
  const recipes = copyAuthoredSourceRecipes(sourceRecipes);
  const createElement = realm.document.createElement.bind(realm.document);
  const canvasRealm = { document: { createElement } };
  const paint = recipe => paintAuthoredCanvas(canvasRealm, recipe);
  const palettes = recipes.palettes.map(paint);
  const bodySources = {
    palettes: recipes.bodySources.palettes.map(palette => ({
      variants: palette.variants.map(paint), boostGlow: paint(palette.boostGlow),
    })),
    effects: Object.fromEntries(Object.entries(recipes.bodySources.effects).map(([key, recipe]) => [key, paint(recipe)])),
  };
  return { palettes, bodySources };
}
