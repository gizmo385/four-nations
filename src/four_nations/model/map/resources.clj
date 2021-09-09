(ns four-nations.model.map.resources
  (:require
    [four-nations.general.utils :as utils]
    [random-seed.core :as rs]))

(defrecord Resource
  [name display-name text-symbol spawn-predicate image])

;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
;;; Validating resource definitions
;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
(defn valid-base-resource?
  "Ensures that the resource has all of the basic required fields."
  [resource]
  (and (:name resource) (:symbol resource) (:predicate resource)))

(defmulti valid-resource-definition?
  "The validation of a resource definition is done based on what kind of predicate the resource
   defines."
  (fn [resource] (:predicate resource)))

(defmethod valid-resource-definition? :default [& _] false)

(defmethod valid-resource-definition? :spawn-on-terrain-types
  [{:keys [spawn-chance terrain-types] :as resource}]
  (and (valid-base-resource? resource) spawn-chance terrain-types))

;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
;;; Building spawn predicates
;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
(defmulti resource-definition->spawn-predicate
  "For a particular resource, builds the spawn predicate function that takes a tile and determines
   if the resource will spawn there."
  (fn [resource] (:predicate resource)))

(defmethod resource-definition->spawn-predicate :default [& _]
  ;; If a resource is declared and the predicate is unknown, we'll ignore it. This should
  ;; get caught in the validation steps.
  (constantly false))

(defmethod resource-definition->spawn-predicate :spawn-on-terrain-types
  [{:keys [terrain-types spawn-chance] :as resource}]
  (fn [tile]
    (let [rand-value (rs/rand)]
      (and (< rand-value spawn-chance)
           (some #{(get-in tile [:attributes :terrain-type])} terrain-types)))))

;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
;;; Parsing the resources EDN file
;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
(defn resource-definition->resource
  "Given an EDN resource definiton, validations that definition and builds the Resource record. This
   resource record will include the tile-based spawn predicate."
  [resource-definition]
  (if (valid-resource-definition? resource-definition)
    (let [spawn-predicate (resource-definition->spawn-predicate resource-definition)]
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
