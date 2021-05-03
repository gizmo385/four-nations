(ns four-nations.model.units.core
  (:require
    [clojure.set :as s]
    [four-nations.general.utils :as utils]))

;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
;;; Record definitions
;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
(defrecord UnitAttribute
  [name display-name description minimum-value maximum-value starting-value])

(defrecord GameUnitType
  [name description tribes attributes])

(defrecord GameUnit
  [id unit-type tribe current-attributes])

;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
;;; Functions for building attributes, unit types, and units
;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
(defn unit-attribute-definitions->unit-attributes-map
  "Given a list of unit attribute definitions, a map of each attribute name to the corresponding
   UnitAttribute."
  [unit-attribute-definitions]
  (->> unit-attribute-definitions
       (map map->UnitAttribute)
       (utils/map-by :name)))

(defn file->unit-attributes
  "Given a filename, loads UnitAttribute definitions out of that file and returns a map of
   each attribute name to the corresponding UnitAttribute"
  [filename]
  (-> filename utils/load-edn-resource unit-attribute-definitions->unit-attributes-map))

(defn unit-type-definition->unit-type
  "Given a raw unit type definition and a map of attribute names to attributes, builds a
   GameUnitType with attached attribute information."
  [unit-type-definition attribute-name->attribute]
  (let [valid-attribute-names (->> attribute-name->attribute vals (map :name) set)
        specified-attribute-names (:attribute-names unit-type-definition)
        attributes (select-keys attribute-name->attribute specified-attribute-names)]
    (if (s/subset? specified-attribute-names valid-attribute-names)
      (-> unit-type-definition
          (assoc :attributes attributes)
          (dissoc :attribute-names)
          map->GameUnitType)
      (throw (ex-info "Invalid attribute for unit type."
                      {:unit-type-definition unit-type-definition})))))

(defn file->unit-types
  "Given a filename and a map of attribute names to the attribute definition, loads GameUnitType
   definitions out of that file."
  [filename attribute-name->attribute]
  (let [raw-unit-types (utils/load-edn-resource filename)]
    (for [ut raw-unit-types]
      (unit-type-definition->unit-type ut attribute-name->attribute))))

;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
;;; Useful functions for interacting with units
;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
(defn initialize-unit-attribute
  "Given a unit attribute, builds an initial version of that attribute for a "
  [unit-attribute]
  (-> unit-attribute
      (select-keys [:name :display-name])
      (assoc :value (:starting-value unit-attribute))))

(defn unit-type->unit
  "Builds a new unit in a particular tribe based on a unit type and the map of valid attributes."
  [unit-type tribe]
  (if (some #{tribe} (:tribes unit-type))
    (let [current-attributes (utils/map-vals :starting-value (:attributes unit-type))]
      (map->GameUnit {:id (utils/uuid)
                      :unit-type unit-type
                      :tribe tribe
                      :current-attributes current-attributes}))
    (throw (ex-info "Disallowed tribe" {:unit-type unit-type :tribe tribe}))))

(comment
  (let [attributes (file->unit-attributes "unit-attributes.edn")
        unit-types (file->unit-types "unit-types.edn" attributes)]
    ;(pprint attributes)
    ;(pprint unit-types)
    (pprint (unit-type->unit (first unit-types) :earth))
    )
  )
