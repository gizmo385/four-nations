(ns four-nations.model.units.core-test
  (:require
    [clojure.test :refer [are deftest is testing]]
    [four-nations.general.utils :as utils]
    [four-nations.model.units.core :as units]))

;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
;;; Test data
;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
(def health-attribute-definition
  {:name :health
   :display-name "Health"
   :description "Determines how much life a unit has. When this hits 0, a unit dies."
   :minimum-value 0
   :starting-value 10})

(def chi-attribute-definition
  {:name :chi
   :display-name "Chi"
   :description "Chi is used in the bending of the elements."
   :minimum-value 0
   :starting-value 100})

(def example-unit-attribute-definitions
  [health-attribute-definition
   chi-attribute-definition])

(def example-unit-type-definition
  {:id :basic-water-worker
   :name "Basic Water Worker"
   :description "A unit which can be used to complete manual labor tasks, such as woodcutting."
   :tribes [:water]
   :attribute-names [:health]})

;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
;;; Testing unit attributes
;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
(defn test-example-unit-attributes
  [attributes]
  (is (= 2 (count attributes)))
  (is (map? attributes))
  (is (some? (->> example-unit-attribute-definitions first :name (get attributes))))
  (is (some? (->> example-unit-attribute-definitions second :name (get attributes))))
  (is (= 10 (get-in attributes [:health :starting-value])))
  (is (= 100 (get-in attributes [:chi :starting-value]))))

(deftest test-unit-attribute-definitions->unit-attributes-map
  (testing "Defining unit attributes"
    (let [attributes (units/unit-attribute-definitions->unit-attributes-map
                       example-unit-attribute-definitions)]
      (test-example-unit-attributes attributes))))

(deftest test-file->unit-attributes
  (testing "Reading unit attributes from a file"
    (with-redefs [utils/load-edn-resource (constantly example-unit-attribute-definitions)]
      (let [attributes (units/file->unit-attributes "random-file")]
        (test-example-unit-attributes attributes)))))

;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
;;; Testing unit types
;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
(defn test-example-unit-type
  [unit-type]
  (is (= (:name unit-type) (:name example-unit-type-definition)))
  (is (map? (:attributes unit-type)))
  (is (map? (get-in unit-type [:attributes :health]))))

(deftest test-unit-type-definition->unit-type
  (testing "Defining unit types"
    (let [attributes (units/unit-attribute-definitions->unit-attributes-map
                       example-unit-attribute-definitions)]
      (test-example-unit-type
        (units/unit-type-definition->unit-type example-unit-type-definition attributes)))))

(deftest test-file->unit-types
  (testing "Reading unit types from a file"
    (with-redefs [utils/load-edn-resource (constantly [example-unit-type-definition])]
      (let [attributes (units/unit-attribute-definitions->unit-attributes-map
                         example-unit-attribute-definitions)
            unit-types (units/file->unit-types "random-file" attributes)]
        (is (every? keyword? (keys unit-types)))
        (doseq [[unit-type-id ut] unit-types]
          (is (= (:id ut) unit-type-id))
          (test-example-unit-type ut))))))

;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
;;; Testing unit creation
;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
(deftest test-unit-type->unit
  (testing "Building new units"
    (let [attributes (units/unit-attribute-definitions->unit-attributes-map
                       example-unit-attribute-definitions)
          unit-type (units/unit-type-definition->unit-type example-unit-type-definition attributes)
          valid-unit (units/unit-type->unit unit-type :water)]
      (doseq [attr-name (:attribute-names unit-type)]
        (are [k] (some? (get valid-unit k))
             :first-name :last-name :description :name)
        (is (map? (get-in valid-unit [:attributes attr-name])))
        (is (some? (get-in valid-unit [:current-attributes attr-name])))))))

;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
;;; Testing unit predicates
;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
(deftest unit-predicates
  (let [attributes (units/unit-attribute-definitions->unit-attributes-map
                     example-unit-attribute-definitions)
        unit-type (units/unit-type-definition->unit-type example-unit-type-definition attributes)
        valid-unit (units/unit-type->unit unit-type :water)]
    (testing "Unit satisfies?"
      ;; Unit is not a part of the earth tribe, they're a part of the water tribe
      (is (false? (units/unit-satisfies? valid-unit #(= :earth (:tribe %)))))
      (is (units/unit-satisfies? valid-unit #(= :water (:tribe %))))

      ;; Returning non-boolean data from a predicate results in a false result
      (is (false? (units/unit-satisfies? valid-unit (constantly nil))))

      ;; A nil unit results in a false result
      (is (false? (units/unit-satisfies? nil (constantly true)))))

    (testing "Unit attribute satisfies?"
      ;; The health attribute should be at its starting value
      (is (units/unit-attribute-satisfies?
            valid-unit :health (partial = (:starting-value health-attribute-definition))))
      ;; Unit does not exist
      (is (false? (units/unit-attribute-satisfies?  nil :health (constantly true))))

      ;; Unit lacks attribute
      (is (false? (units/unit-attribute-satisfies?  valid-unit :invalid-attr (constantly true))))

      ;; Predicate returns invalid value
      (is (false? (units/unit-attribute-satisfies?  valid-unit :invalid-attr (constantly nil)))))

    (testing "Helper predicates (dead?, hungry?, thirst?, etc)"
      ;; Invalid units
      (are [pred] (false? (pred nil)) units/thirsty? units/hungry? units/dead?)

      ;; Is a unit thirsty?
      (is (units/thirsty? {:current-attributes {:thirst 0}}))
      (is (units/thirsty? {:current-attributes {:thirst -1}}))
      (is (false? (units/thirsty? {:current-attributes {:thirst 100}})))

      ;; Is a unit hungry?
      (is (units/hungry? {:current-attributes {:hunger 0}}))
      (is (units/hungry? {:current-attributes {:hunger -1}}))
      (is (false? (units/hungry? {:current-attributes {:hunger 100}})))

      ;; Is a unit dead?
      (is (units/dead? {:current-attributes {:health 0}}))
      (is (units/dead? {:current-attributes {:health -1}}))
      (is (false? (units/dead? {:current-attributes {:health 100}}))))))
