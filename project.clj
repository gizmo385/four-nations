(defproject gizmo385/discord.clj "0.0.1-SNAPSHOT"
  :description "Settlement management game in Clojure"
  :url "https://github.com/gizmo385/settlement-management.cl"
  :license {:name "Eclipse Public License"
            :url "http://www.eclipse.org/legal/epl-v10.html"}
  :dependencies [[org.clojure/clojure "1.10.0"]
                 [cljfx/cljfx                             "1.7.13"]
                 [clojure-term-colors/clojure-term-colors "0.1.0"]
                 [faker/faker                             "0.3.2"]
                 [org.clojure/clojure                     "1.11.0-alpha1"]
                 [org.clojure/core.cache                  "1.0.207"]
                 [org.clojure/tools.cli                   "1.0.206"]
                 [random-seed/random-seed                 "1.0.0"]]
  :main ^:skip-aot four-nations.view.gui.core
  :target-path "target/%s"
  :source-paths ["src" "dev"]
  :resource-paths ["resources"]
  :repl-options {:init-ns user}
  :profiles {:uberjar {:aot :all}
             :map-gen {:main ^:skip-aot four-nations.model.map}})
