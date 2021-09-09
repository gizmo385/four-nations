(ns four-nations.model.civilization-test
  (:require
    [clojure.test :refer [deftest is testing]]
    [four-nations.model.civilization :as civ]))

;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
;;; Defining some test data
;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
(def thirst-attribute
  {:name :thirst
   :display-name "Thirst"
   :description "Describes how in-need of water a unit is."
   :minimum-value 0
   :maximum-value 100
   :starting-value 0})

(def health-attribute
  {:name :health
   :display-name "Health"
   :description "Determines how much life a unit has. When this hits 0, a unit dies."
   :minimum-value 0
   :starting-value 10})

(def test-unit-attributes {:health health-attribute :thirst thirst-attribute})

(def test-unit-types
  {:basic-worker {:id :basic-worker
                  :name "Basic Worker"
                  :description "A unit which can be used to complete manual labor tasks, such as woodcutting."
                  :tribes [:water :air :fire :earth]
                  :attribute-names [:health :thirst]}})

(def test-civilization-metadata
  (civ/->CivilizationMetadata test-unit-attributes test-unit-types))

;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
;;; Testing that unit attribute bounds (min/max) are respected.
;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
(deftest attribute-bounds-test
  (let [civilization (-> (civ/start-civilization test-civilization-metadata :water)
                         (civ/spawn-unit :basic-worker 0 0))
        unit-id (-> civilization :units keys first) ]
    (testing "Updating a bounded unit attribute safely obeys bounds."
      ;; Going under the minimum bound
      (is (-> civilization
              (civ/set-unit-attribute unit-id :health (+ (:minimum-value health-attribute) 10))
              (civ/update-unit-attribute unit-id :health - 15)
              (get-in [:units unit-id :current-attributes :health])
              (= (:minimum-value health-attribute))))
      ;; Going above the maximum bound
      (-> civilization
          (civ/set-unit-attribute unit-id :thirst (- (:maximum-value thirst-attribute) 10))
          (civ/update-unit-attribute unit-id :thirst + 15)
          (get-in [:units unit-id :current-attributes])
          (= (:maximum-value thirst-attribute))))))

(deftest unit-predicate-checks
  (let [civilization (-> (civ/start-civilization test-civilization-metadata :water)
                         (civ/spawn-unit :basic-worker 0 0))
        unit-id (-> civilization :units keys first)]
    (testing "Unit death checks"
      (is (not (civ/unit-dead? civilization unit-id)))
      (is (-> civilization (civ/set-unit-attribute unit-id :health 0) (civ/unit-dead? unit-id))))
    (testing "Unit satisfies"
      (is (civ/unit-satisfies? civilization unit-id #(-> % :id (= unit-id)))))))

(deftest remove-unit-test
  (testing "Removing a unit works"
    (let [civilization (-> (civ/start-civilization test-civilization-metadata :water)
                           (civ/spawn-unit :basic-worker 0 0))
          unit-id (-> civilization :units keys first)]
      (is (-> civilization (civ/remove-unit unit-id) :units count zero?)))))
