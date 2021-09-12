(ns four-nations.model.units.core
  (:require
    [clojure.set :as s]
    [faker.name :as fake]
    [four-nations.general.utils :as utils]))

;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
;;; Record definitions
;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
(defrecord UnitAttribute
  [name display-name description minimum-value maximum-value starting-value])

(defrecord GameUnitType
  [id name description tribes attributes image])

(defrecord GameUnit
  [id unit-type tribe current-attributes first-name last-name])

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
    (->> (for [ut raw-unit-types]
           (unit-type-definition->unit-type ut attribute-name->attribute))
         (utils/map-by :id))))

;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
;;; Creating units
;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
(defn unit-type->unit
  "Builds a new unit in a particular tribe based on a unit type and the map of valid attributes."
  [unit-type tribe]
  (if (some #{tribe} (:tribes unit-type))
    (let [current-attributes (utils/map-vals :starting-value (:attributes unit-type))]
      (map->GameUnit {:id (utils/uuid)
                      :unit-type unit-type
                      :tribe tribe
                      :current-attributes current-attributes
                      :first-name (fake/first-name)
                      :last-name (fake/last-name)}))
    (throw (ex-info "Disallowed tribe" {:unit-type unit-type :tribe tribe}))))

;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
;;; Checking the status of units
;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
(defn unit-satisfies?
  "Checks that the unit satisfies the supplied predicate. The predicate should take the unit as a
   single argument and return a boolean. If the unit is nil, the predicate will not be called and
   false will be returned."
  [unit pred]
  (boolean (when unit (pred unit))))

(defn unit-attribute-satisfies?
  "Checks if a units current attribute value satisifes the supplied predicate. The predicate should
   take the current value as the first argument. Any additional arguments supplied to this function
   will be supplied to the predicate as secondary arguments. If the specified unit lacks the
   specified attribute, the predicate will NOT be called and false will be returned."
  [unit attribute pred & args]
  (if-let [current-value (get-in unit [:current-attributes attribute])]
    (apply pred current-value args)
    false))

(defn dead?
  "Returns true if the unit's health attribute is less than or equal to 0, false otherwise. If the
   unit does not have a health attribute, false will be returned."
  [unit]
  (unit-attribute-satisfies? unit :health <= 0))

(defn thirsty?
  "Returns true if the unit's thirst attribute is less than or equal to 0, false otherwise. If the
   unit does not have a thirst attribute, false will be returned."
  [unit]
  (unit-attribute-satisfies? unit :thirst <= 0))

(defn hungry?
  "Returns true if the unit's hunger attribute is less than or equal to 0, false otherwise. If the
   unit does not have a hunger attribute, false will be returned."
  [unit]
  (unit-attribute-satisfies? unit :hunger <= 0))
