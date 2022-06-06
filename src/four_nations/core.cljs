(ns four-nations.core
  (:require
    [four-nations.model.map :as m]
    [four-nations.model.map.noise-map :as nm]
    [four-nations.model.map.utils :as map-utils]
    [four-nations.general.types :as types]
    [reagent.core :as reagent :refer [atom]]
    [reagent.dom :as rd]))

(enable-console-print!)

;; define your app data so that it doesn't get over-written on reload

(defonce app-state (atom {:text "Hello world!"}))

(defn map-tile
  [t]
  [:a
   {:style {:height 30
            :width 30
            :border "1px solid black"
            :background-color (-> t :attributes :terrain-type map-utils/terrain-type->color)}}
   (map-utils/tile->printable-char t)])

(defn game-map-element
  [game-map dimension]
  (let [res [:div
             [:div
              (for [y (range (:height dimension))]
                [:div
                 (for [x (range (:width dimension))]
                   [map-tile
                    (-> game-map (map-utils/get-cell (types/->Point x y)))])
                 [:br]])]]]
    (println res)
    res)
  )


(defn hello-world []
  (let [d (types/->Dimension 50 200)
        game-map (time (m/build-map d 20 {})) ]
    [game-map-element game-map d]))

(rd/render [hello-world]
           (. js/document (getElementById "app")))

(defn on-js-reload []
  ;; optionally touch your app-state to force rerendering depending on
  ;; your application
  ;; (swap! app-state update-in [:__figwheel_counter] inc)
  )
