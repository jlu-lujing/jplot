# extract_api.R — dump ggplot2's complete API surface to compare/api_inventory.json
#
# For every geom/stat/position/coord/facet/scale/guide/theme function:
#   full signature with deparsed defaults;
# For every Geom/Stat ggproto: required_aes, non_missing_aes, default_aes
#   (constants deparsed), extra_params. For theme elements: resolved values
#   under every shipped ggplot2 theme (what each theme CHANGES).
suppressMessages(library(ggplot2))
.here <- tryCatch({.a <- commandArgs(FALSE); .f <- .a[grepl("^--file=", .a)][1]; dirname(normalizePath(sub("^--file=", "", .f)))}, error=function(e) NULL)
if (!is.null(.here)) setwd(.here)
suppressMessages(library(jsonlite))

ns <- asNamespace("ggplot2")
dep <- function(x) {
  if (is.null(x)) return("NULL")
  if (is.character(x) && length(x) == 1) return(paste0('"', x, '"'))
  tryCatch(paste(deparse(x, width.cutoff = 500L), collapse = " "), error = function(e) "«obj»")
}

# ---- constructors: geom_*, stat_*, position_*, coord_*, facet_*, scale_*,
#      guides, guide_*, labs, theme*, annotate, annotation_*, margin, aes, ...
funcs <- ls(ns)
sel <- grep("^(geom_|stat_|position_|coord_|facet_|scale_|guide_|guides|labs$|theme_|element_|aes$|annotate$|annotation_|after_stat$|after_scale$|waiver$|expansion$|mspline$)", funcs, value = TRUE)
fninfo <- lapply(sel, function(f) {
  obj <- get(f, envir = ns)
  if (!is.function(obj)) return(NULL)
  fm <- formals(args(obj))
  if (length(fm) == 0) return(NULL)
  list(defaults = lapply(fm, dep))
})
names(fninfo) <- sel
fninfo <- Filter(Negate(is.null), fninfo)

# ---- ggproto objects -------------------------------------------------------
ggos <- grep("^(Geom|Stat|Position|Coord|Facet|Guide)", ls(ns), value = TRUE)
ggos <- ggos[!grepl("^Geom$|^Stat$|^Position$|^Coord$|^Facet$|^Guide$", ggos)]
gginfo <- lapply(ggos, function(n) {
  o <- get(n, envir = ns)
  if (!inherits(o, "ggproto")) return(NULL)
  dflt <- tryCatch({
    da <- o$default_aes
    if (is.null(da)) list() else lapply(as.list(da), dep)
  }, error = function(e) list())
  list(
    required_aes = unlist(o$required_aes),
    non_missing_aes = unlist(o$non_missing_aes),
    optional_aes = unlist(o$optional_aes),
    extra_params = unlist(o$extra_params),
    default_aes = dflt
  )
})
names(gginfo) <- ggos
gginfo <- Filter(Negate(is.null), gginfo)

# ---- scale constructors grouped by aesthetic (x,y,colour,fill,size,...) ----
scale_fns <- grep("^scale_", sel, value = TRUE)
# ---- themes: what each theme_grey variant CHANGES --------------------------
themes <- grep("^theme_(grey|gray|bw|light|dark|minimal|classic|void|linedraw|panel)", ls(ns), value = TRUE)
elem_names <- c("text","title","axis.text","axis.text.x","axis.text.y","axis.title",
  "axis.title.x","axis.title.y","axis.ticks","axis.ticks.length","axis.line",
  "panel.background","panel.border","panel.grid.major","panel.grid.minor",
  "panel.spacing","plot.title","plot.subtitle","plot.caption",
  "legend.position","legend.text","legend.title","legend.key.width","legend.box")
themevals <- lapply(themes, function(th) {
  base <- tryCatch(get(th, envir = ns)(), error = function(e) NULL)
  if (is.null(base)) return(NULL)
  out <- list()
  for (el in elem_names) {
    v <- tryCatch(calc_element(el, base), error = function(e) NULL)
    if (is.null(v)) next
    if (inherits(v, "element_text")) {
      out[[el]] <- list(type="text", size=v$size, colour=v$colour, face=v$face %||% "plain",
                        hjust=v$hjust, vjust=v$vjust, angle=v$angle,
                        margin=paste(v$margin %||% rep(NA,4), collapse=","))
    } else if (inherits(v, "element_line")) {
      out[[el]] <- list(type="line", colour=v$colour, linewidth=v$linewidth, linetype=v$linetype)
    } else if (inherits(v, "element_rect")) {
      out[[el]] <- list(type="rect", fill=v$fill, colour=v$colour, linewidth=v$linewidth)
    } else if (inherits(v, "element_blank")) {
      out[[el]] <- list(type="blank")
    } else if (is.numeric(v)) {
      out[[el]] <- list(type="num", value=as.numeric(v))
    } else if (is.character(v)) {
      out[[el]] <- list(type="str", value=v)
    }
  }
  out
})
names(themevals) <- themes
themevals <- Filter(Negate(is.null), themevals)

# ---- discrete palettes used by default hue scale (first 9 hexes) -----------
suppressMessages(library(scales))
pal <- lapply(1:9, function(n) hue_pal()(n))
names(pal) <- as.character(1:9)

cat("exported:", length(sel), " constructors;", length(gginfo), " ggproto; themes:", length(themevals), "\n")
jsonlite::write_json(
  list(functions = fninfo, ggproto = gginfo, themes = themevals,
       theme_elements = elem_names, hue_palette = pal,
       version = as.character(packageVersion("ggplot2"))),
  "../api_inventory.json", auto_unbox = TRUE, pretty = TRUE, na = "null")
