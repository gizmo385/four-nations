// Compiled by ClojureScript 1.10.773 {}
goog.provide('four_nations.model.map');
goog.require('cljs.core');
goog.require('four_nations.model.map.noise_map');
goog.require('four_nations.model.map.enrichments');
/**
 * Builds a noisemap and then enriches that noise map to form a final game map.
 */
four_nations.model.map.build_map = (function four_nations$model$map$build_map(dimensions,smoothing_passes,enrichment_options){
var final_enrichment_options = cljs.core.merge.call(null,four_nations.model.map.enrichments.default_enrichment_options,enrichment_options);
cljs.core.println.call(null,final_enrichment_options);

return four_nations.model.map.enrichments.enrich_noise_map.call(null,four_nations.model.map.noise_map.generate_noisemap.call(null,dimensions,smoothing_passes),dimensions,final_enrichment_options);
});

//# sourceMappingURL=map.js.map?rel=1654487548447
