(ns four-nations.general.utils-test
  (:require
    [clojure.test :refer [deftest is testing]]
    [four-nations.general.utils :as utils]))

(deftest test-map-vals
  (testing "Mapping over values"
    (is (->> {:a 1 :b 2 :c 3}
             (utils/map-vals inc)
             (= {:a 2 :b 3 :c 4})))))

(deftest test-map-by
  (testing "Mapping a list of maps to one of the map keys"
    (is (->> [{:a 1} {:a 2 :b 2}]
             (utils/map-by :a)
             (= {1 {:a 1} 2 {:a 2 :b 2}})))))

(deftest test-n-times
  (testing "Running a function on a value n times"
    (is (= 5 (utils/n-times inc 0 5)))))
