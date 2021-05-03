(ns four-nations.model.units.core-test
  (:require
    [clojure.test :refer [are deftest is testing]]
    [four-nations.general.utils :as utils]
    [four-nations.model.units.core :as units]))

;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
;;; Test data
;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
(def example-unit-attribute-definitions
  [{:name :health
    :display-name "Health"
    :description "Determines how much life a unit has. When this hits 0, a unit dies."
    :minimum-value 0
    :starting-value 10}
   {:name :chi
    :display-name "Chi"
    :description "Chi is used in the bending of the elements."
    :minimum-value 0
    :starting-value 100}])

(def example-unit-type-definition
  {:name "Basic Worker"
   :description "A unit which can be used to complete manual labor tasks, such as woodcutting."
   :tribes [:water :air :fire :earth]
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
        (doseq [ut unit-types]
          (test-example-unit-type ut))))))

;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
;;; Testing unit creation
;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
(deftest test-initialize-unit-attribute)

(deftest test-unit-type->unit)
