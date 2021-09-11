(ns four-nations.model.map.resources
  (:require
    [four-nations.general.utils :as utils]
    [random-seed.core :as rs]))

(defrecord Resource
  [name display-name text-symbol spawn-predicate image])

;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
;;; Validating resource definitions
;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
(defn valid-resource-definition?
  "Ensures that the resource has all of the basic required fields."
  [resource]
  (and (:name resource) (:symbol resource) (:spawning resource)))

;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
;;; Determining if a resource can spawn on a particular tile
;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
(defn spawn-resource-on-tile?
  [resource tile]
  (let [spawn-chance (get-in resource [:spawning :chance])
        allowed-biomes (get-in resource [:spawning :attributes :biomes])
        allowed-terrain-types (get-in resource [:spawning :attributes :terrain-types])
        tile-biome (get-in tile [:attributes :biome :name])
        tile-terrain-type (get-in tile [:attributes :terrain-type])]
    (and (< (rs/rand) spawn-chance)
         (or (nil? allowed-biomes)
             (contains? allowed-biomes tile-biome))
         (or (nil? allowed-terrain-types)
             (contains? allowed-terrain-types tile-terrain-type)))))

;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
;;; Parsing the resources EDN file
;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
(defn resource-definition->resource
  "Given an EDN resource definiton, validations that definition and builds the Resource record. This
   resource record will include the tile-based spawn predicate."
  [resource-definition]
  (if (valid-resource-definition? resource-definition)
    (let [spawn-predicate (partial spawn-resource-on-tile? resource-definition)]
      (map->Resource (assoc resource-definition :spawn-predicate spawn-predicate)))
    (throw (ex-info "Invalid resource definition!" resource-definition))))

(defn load-resource-definitions
  "Given a file with EDN resource definitions, returns resource records."
  [filename]
  (->> filename
       utils/load-edn-resource
       (map resource-definition->resource)))

(comment
  (use 'clojure.pprint)
  (pprint (load-resource-definitions "resources.edn"))
  )
