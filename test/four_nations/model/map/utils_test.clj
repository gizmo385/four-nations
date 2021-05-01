(ns four-nations.model.map.utils-test
  (:require
    [four-nations.model.map.utils :as utils]
    [clojure.test :refer [are deftest is testing]]))


(deftest test-valid-coordinate
  (testing "Valid coordinates"
    (are [coord] (utils/valid-coordinate? 10 10 coord)
         [0 0] [1 1] [4 4] [9 9]))
  (testing "Invalid coordinates"
    (are [coord] (not (utils/valid-coordinate? 10 10 coord))
         [-1 0] [0 -1] [0 10] [10 0])))

(deftest test-get-cell
  (let [m [[1 2 3]
           [4 5 6]
           [7 8 9]]]
    (testing "Retrieving valid cells"
      (is (= (utils/get-cell m 0 0) 1))
      (is (= (utils/get-cell m 1 0) 2))
      (is (= (utils/get-cell m 0 1) 4))
      (is (= (utils/get-cell m 2 2) 9)))
    (testing "Retrieving invalid cells"
      (is (nil? (utils/get-cell m -1 0)))
      (is (nil? (utils/get-cell m 0 -1)))
      (is (nil? (utils/get-cell m 3 2))))))

(deftest test-coordinates->neighbor-coordinates
  (testing "All neighbors"
    (are [coord expected-neighbors] (= (set expected-neighbors)
                                       (utils/coordinates->neighbor-coordinates 10 10 coord))
         [0 0] [[1 1] [0 1] [1 0]]
         [1 1] [[0 0] [0 1] [1 0] [0 2] [2 0] [2 1] [1 2] [2 2]]
         [9 9] [[8 9] [9 8] [8 8]])
    (are [coord expected-neighbors] (= (set expected-neighbors)
                                       (utils/coordinates->neighbor-coordinates 10 10 coord true))
         [0 0] [[1 1] [0 1] [1 0]]
         [1 1] [[0 0] [0 1] [1 0] [0 2] [2 0] [2 1] [1 2] [2 2]]
         [9 9] [[8 9] [9 8] [8 8]]))
  (testing "Cardinal neighbors"
    (are [coord expected-neighbors] (= (set expected-neighbors)
                                       (utils/coordinates->neighbor-coordinates 10 10 coord false))
         [0 0] [[0 1] [1 0]]
         [1 1] [[0 1] [1 0] [2 1] [1 2]]
         [9 9] [[8 9] [9 8]])))

(deftest test-two-dimensional-map
  (testing "Mapping a function over every cell"
    ;; Build a 3x3 map of 0s and increment each cell. Should result in a 3x3 map of 1s.
    (is (= (utils/two-dimensional-map
             (fn [v x y] (inc v)) (repeat 3 (repeat 3 0)) 3 3))
        (repeat 3 (repeat 3 1)))))
