(ns four-nations.model.civilization
  (:require
    [four-nations.model.units.core :as units]))

;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
;;; Civilization creation
;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;

;;; The civilization metadata is information that is useful to keep track of but is not immediately
;;; relevant to the state of the civilization. This includes information that has been loaded from
;;; static configuration files, such as the available unit attributes and unit types.
(defrecord CivilizationMetadata
  [unit-attributes unit-types])

(defrecord Civilization
  [units unit-locations tribe metadata])

(defn load-civilization-metadata
  [unit-attributes-filename unit-types-filename]
  (let [unit-attributes (units/file->unit-attributes unit-attributes-filename)
        unit-types (units/file->unit-types unit-types-filename unit-attributes)]
    (->CivilizationMetadata unit-attributes unit-types)))

(defn start-civilization
  [metadata tribe]
  (->Civilization {} {} tribe metadata))

(defn add-unit
  [civilization unit x y]
  (let [unit-id (:id unit)]
    (-> civilization
        (assoc-in [:units unit-id] unit)
        (assoc-in [:unit-locations unit-id] [x y]))))

(defn spawn-unit
  [civilization unit-type x y]
  (let [unit-type-details (get-in civilization [:metadata :unit-types unit-type])
        new-unit (units/unit-type->unit unit-type-details (:tribe civilization))]
    (add-unit civilization new-unit x y)))

;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
;;; Checking the state of a civilization or the units within.
;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
(defn unit-satisfies?
  "Checks the unit against the supplied predicate. The predicate should take the current state of the
   unit as a single argument and return a boolean. If the unit with the specified ID does not exist,
   the predicate will NOT be called and nil will be returned."
  [civilization unit-id pred]
  (when-let [unit (get-in civilization [:units unit-id])]
    (pred unit)))

(defn unit-attribute-satisfies?
  "Checks if a units current attribute value satisifes the supplied predicate. The predicate should
   take the current value as the first argument. Any additional arguments supplied to this function
   will be supplied to the predicate as secondary arguments. If the specified unit does not exist,
   or lacks the specified attribute, the predicate will NOT be called and nil will be returned."
  [civilization unit-id attribute pred & args]
  (when-let [current-value (get-in civilization [:units unit-id :current-attributes attribute])]
    (apply pred current-value args)))

(defn unit-dead?
  "Checks if the a unit is dead, which occurs when their current health has reached 0. If the unit
   doesn't exist or does not have the :health attribute, nil is returned."
  [civilization unit-id]
  (unit-attribute-satisfies? civilization unit-id :health = 0))

;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
;;; Modifying a civilization or the units within.
;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
(defn set-unit-location
  "Updates the location of a unit within a civilization to a newly specified x/y."
  [civilization unit-id new-x new-y]
  (assoc-in civilization [:unit-locations unit-id] [new-x new-y]))

(defn move-unit
  "Moves a unit by the specified delta-x/delta-y. If that unit does not exist, the existing
   civilization is returned."
  [civilization unit-id delta-x delta-y]
  (if-let [[current-x current-y] (get-in civilization [:unit-locations unit-id])]
    (set-unit-location civilization unit-id (+ current-x delta-x) (+ current-y delta-y))
    civilization))

(defn set-unit-attribute
  "Sets the value of a unit's attribute to the specified value. This can override limits for an
   attribute, so for most situations `bounded-set-unit-attribute` should be preferred.."
  [civilization unit-id attribute new-value]
  (assoc-in civilization [:units unit-id :current-attributes attribute] new-value))

(defn bounded-set-unit-attribute
  "Sets the value of a unit's attribute to the specified value, or the nearest bounding value for
   that attribute (maximum/minimum)."
  [civilization unit-id attribute new-value]
  (let [attribute-spec (get-in civilization [:metadata :unit-attributes attribute])
        min-value (:minimum-value attribute-spec)
        max-value (:maximum-value attribute-spec)]
    (assoc-in
      civilization
      [:units unit-id :current-attributes attribute]
      (cond-> new-value
        (some? min-value) (max new-value min-value)
        (some? max-value) (min new-value max-value)))))

(defn update-unit-attribute
  "Updates a unit's attribute within a civilization, as if by update-in, with the function f, the
   current value of the attribute, and any other additional arguments."
  [civilization unit-id attribute f & args]
  (let [current-value (get-in civilization [:units unit-id :current-attributes attribute])
        new-value (apply f current-value args)]
    (bounded-set-unit-attribute civilization unit-id attribute new-value)))

(defn remove-unit
  "Removes the unit with the specified ID from the civilization"
  [civilization unit-id]
  (-> civilization
      (update :units dissoc unit-id)
      (update :unit-locations dissoc unit-id)))
