(ns four-nations.model.map.enrichments
  (:require
    [four-nations.model.map.biomes :as biomes]
    [four-nations.model.map.utils :as utils]
    [four-nations.model.map.resources :as res]
    [four-nations.general.types :refer [->Dimension]]
    ))

;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
;;; Helper functions
;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
(defn average-map-value
  "Given a noise map, calculates the average value of all of the cells within that noise map."
  [noise]
  (let [all-values (map :value (vals noise))]
    (quot (apply + all-values)
          (count all-values))))

;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
;;; Initial map tile creation
;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
(defn determine-cell-terrain-type
  "Determines if a cell should be land, water, or mountain based on the cell value's relation to
   the average value of the game map."
  [cell-value average-value]
  (if (< cell-value average-value) :water :land))

;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
;;; Various enrichments to the game map, such as adding coastline or placing resources
;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
(defn water-should-spread-to-tile?
  "Determines if water should spread to a particular tile, based on the terrain types of the
   neighbors and a random chance."
  [tile m dimensions spread-chance]
  (and (utils/some-neighbors-are? (partial utils/has-terrain-type? :water) tile m dimensions)
       (utils/has-terrain-type? :land tile)
       (< (rand) spread-chance)))

(defn spread-water
  "Given a game map and spread chance, for every land tile within the game that is near water, the
   spread chance determines how likely it is that the water will spread to that tile."
  [m dimensions {:keys [water-spread-chance]}]
  (utils/map-over-tiles
    (fn [tile]
      (if (water-should-spread-to-tile? tile m dimensions water-spread-chance)
        (assoc-in tile [:attributes :terrain-type] :water)
        tile))
    m
    dimensions))

(defn tile-should-be-drenched?
  "Given a tile, a map, and its dimensions, determines if that tile should be drenched with water,
   which happens when a land tile is surrounded by water tiles."
  [tile m dimensions]
  (and (utils/all-neighbors-are? (partial utils/has-terrain-type? :water) tile m dimensions)
       (utils/has-terrain-type? :land tile)))

(defn drench-surrounded-land
  "Given a game map, finds any land tiles where all of its cardinal neighbors are water and sets
   those tiles to also be water. This prevents tiny, nonsensical islands."
  [m dimensions]
  (utils/map-over-tiles
    (fn [tile]
      (if (tile-should-be-drenched? tile m dimensions)
        (assoc-in tile [:attributes :terrain-type] :water)
        tile))
    m
    dimensions))

(defn should-be-coastline?
  "Returns true if the specified tile should be coastline."
  [tile m dimensions]
  (and (utils/has-terrain-type? :land tile)
       (utils/some-neighbors-are? (partial utils/has-terrain-type? :water) tile m dimensions)))

(defn add-coastline
  "Given a map, changes the terrain type of any land tiles that are near water to be coastline."
  [m dimensions]
  (utils/map-over-tiles
    (fn [tile]
      (if (should-be-coastline? tile m dimensions)
        (assoc-in tile [:attributes :terrain-type] :coast)
        tile))
    m
    dimensions))

(defn add-water-border
  "Adds a water border, where all tiles along the edge of the map become water, of a specified size
   to the map."
  [m dimensions {:keys [water-border]}]
  (let [water-width-edge (- (:width dimensions) water-border)
        water-height-edge (- (:height dimensions) water-border)]
    (map
      (fn [[{:keys [x y] :as point} tile]]
        [point
         (if (or (< x water-border)
                 (>= x water-width-edge)
                 (< y water-border)
                 (>= y water-height-edge))
           (assoc-in tile [:attributes :terrain-type] :water)
           tile)])
      m)))

(defn maybe-add-resource-to-tile
  [tile {:keys [spawn-predicate] :as resource}]
  (if (and (nil? (get-in tile [:attributes :resource]))
           (spawn-predicate tile))
    (assoc-in tile [:attributes :resource] resource)
    tile))

(defn add-resources
  "Given a map, its dimensions and available resources; adds resources to the map based on the
   specified spawn predicates for those resources."
  [m dimensions {:keys [resources]}]
  (utils/map-over-tiles
    (fn [tile]
      (reduce maybe-add-resource-to-tile tile resources))
    m
    dimensions))

(defn add-basic-terrain
  "Given the noise map and the average value of cells across the map, builds basic game map with
   simple terrain selection."
  [noise average-value]
  (map (fn [[point tile]]
         (let [terrain-type (determine-cell-terrain-type (:value tile) average-value)]
           [point (assoc-in tile [:attributes :terrain-type] terrain-type)]))
       noise))

(defn add-biomes
  "Adds biomes to the map through random subdivision."
  [m dimensions {:keys [biomes biome-count]}]
  (biomes/add-biomes m dimensions biome-count biomes))

;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
;;; Main entry point for taking a noise map and adding enrichments to it
;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
(def default-biomes
  (map
    biomes/map->Biome
    [{:name :plains
      :display-name "Plains"}
     {:name :forest
      :display-name "Forest"}
     {:name :desert
      :display-name "Desert"}
     {:name :tundra
      :display-name "Tundra"}]))

(def default-resources
  (map
    res/resource-definition->resource
    [{:name :gold-vein
      :display-name "Gold Vein"
      :symbol "$"
      :spawning {:chance 0.01
                 :attributes {:terrain-types #{:land :mountain}}}
      :image "images/resources/gold.png"}
     {:name :oak-tree
      :display-name "Oak Tree"
      :symbol "T"
      :spawning {:chance 0.30
                 :attributes {:terrain-types #{:land}
                              :biomes #{:plains}}}
      :image :oak-tree}
     {:name :fir-tree
      :display-name "Fir Tree"
      :symbol "F"
      :spawning {:chance 0.90
                 :attributes {:terrain-types #{:land}
                              :biomes #{:forest}}}
      :image :fir-tree}
     {:name :oil-deposit
      :display-name "Oil Deposit"
      :symbol "O"
      :spawning {:chance 0.005
                 :attributes {:terrain-types #{:water}}}
      :image "images/resources/oil.png"}]))

(def default-enrichment-options
  {:water-spread-chance 0.1
   :water-border 2
   :biome-count (* 5 (count default-biomes))
   :biomes default-biomes
   :resources default-resources})

(defn enrich-noise-map
  "Given a generated and smoothed noisemap, generates a game map with additional attributes."
  ([noise dimensions]
   (enrich-noise-map noise dimensions default-enrichment-options))

  ([noise dimensions options]
   (let [average-value (average-map-value noise)]
     (-> noise
         (add-basic-terrain average-value)
         (add-biomes dimensions options)
         (add-water-border dimensions options)
         (spread-water dimensions options)
         (drench-surrounded-land dimensions)
         (add-coastline dimensions)
         (add-resources dimensions options)))))

;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
;;; Rich comments for experimenting :)
;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
(comment
  (require
    '[four-nations.model.map.noise-map :as nm]
    '[clojure.pprint :refer [pprint]])

  (let [dimensions (->Dimension 175 40)
        smoothing-passes 5
        noise (nm/generate-noisemap dimensions smoothing-passes)
        enriched-map (enrich-noise-map noise dimensions)]
    (utils/print-map enriched-map dimensions)
    )
  )
