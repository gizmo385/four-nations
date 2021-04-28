(ns four-nations.general.utils)

(defn n-times
  "Given a function f, an initial value, and a number n, performs n calls of f. The initial value is"
  [f init n]
  (reduce
    (fn [value _idx]
      (f value))
    init
    (range n)))
