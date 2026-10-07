# export_specs.R — writes jplot spec JSON + ggplot2 reference PNGs, same data.
# Run: Rscript export_specs.R  (in this directory)
# Refs rendered at 72 dpi: width=720/72 in, height=480/72 in.
suppressMessages(library(ggplot2))

# resolve paths relative to this script, so cwd is irrelevant
.here <- tryCatch({
  .a <- commandArgs(FALSE)
  .f <- .a[grepl("^--file=", .a)][1]
  dirname(normalizePath(sub("^--file=", "", .f)))
}, error = function(e) NULL)
if (!is.null(.here)) setwd(.here)

dir.create("specs", showWarnings = FALSE)
dir.create("refs", showWarnings = FALSE)
dir.create("ours", showWarnings = FALSE)
set.seed(42)

q2 <- function(v) if (is.numeric(v) && any(!is.finite(v))) { stop("non-finite") }
jcol <- function(v) {
  if (is.factor(v)) {
    list(kind = "categorical", values = as.character(v), levels = levels(v))
  } else if (is.character(v)) {
    list(kind = "categorical", values = v)
  } else {
    list(kind = "numeric", values = as.numeric(v))
  }
}
jdata <- function(df) {
  setNames(lapply(df, jcol), names(df))
}
spec <- function(data, mapping, layers, scales = list(), labels = list(), theme = list(kind = "grey"),
                 width = 720, height = 480) {
  list(
    data = jdata(data),
    mapping = list(map = mapping),
    layers = layers,
    scales = scales,
    labels = labels,
    theme = theme,
    width = width, height = height
  )
}
layer <- function(geom, ..., stat = NULL, position = list(kind = "identity"), mapping = NULL, args = list()) {
  g <- switch(geom,
    point = list(kind = "point"),
    line = list(kind = "line"),
    col = list(kind = "col"),
    bar = list(kind = "bar"),
    histogram = list(kind = "histogram", bins = 30),
    boxplot = list(kind = "boxplot"),
    freqpoly = list(kind = "freqpoly", bins = 30),
    step = list(kind = "step"),
    hline = list(kind = "hline"),
    vline = list(kind = "vline"),
    text = list(kind = "text"),
    area = list(kind = "area"),
    errorbar = list(kind = "errorbar"),
    ribbon = list(kind = "ribbon")
  )
  list(geom = g, stat = stat, position = position,
       mapping = if (is.null(mapping)) list(map = list()) else list(map = mapping),
       args = args, inherit_aes = TRUE)
}

save <- function(name, sp) {
  jsonlite::write_json(sp, file.path("specs", paste0(name, ".json")), auto_unbox = TRUE, digits = 10, null = "null", empty_object = TRUE)
}
render <- function(name, p) {
  # SVG via svglite → rasterised by jplot's own resvg in the compare step, so
  # reference and jplot share ONE rasteriser + ONE font face (font differences
  # would otherwise swamp geometry differences).
  svgf <- file.path("refs", paste0(name, ".svg"))
  svglite(svgf, width = 720/72, height = 480/72, pointsize = 11)
  print(p)
  invisible(dev.off())
  # svglite writes width='720.00pt' → resvg reads pt at 96dpi (960px), which
  # mismatches our 720px canvas; and its default round linecap fattens short
  # strokes vs ggplot2's cairo butt caps. Normalise both so resvg reproduces
  # the exact ggplot2 cairo geometry.
  svg_txt <- readLines(svgf, warn = FALSE)
  svg_txt <- gsub("([0-9.]+)pt'", "\\1'", svg_txt)
  svg_txt <- gsub("stroke-linecap: round", "stroke-linecap: butt", svg_txt, fixed = TRUE)
  writeLines(svg_txt, svgf)
  png(file.path("refs", paste0(name, ".png")), width = 720, height = 480, res = 72, type = "cairo")
  print(p)
  invisible(dev.off())
}

suppressMessages(library(jsonlite))
suppressMessages(library(svglite))

df <- mtcars
df$cyl <- factor(df$cyl)
df$vs <- factor(df$vs)
df$gear <- factor(df$gear)
df$am <- factor(df$am)
df$name <- rownames(df)

## 01 scatter -------------------------------------------------------------
save("01_scatter", spec(df, list(x = "disp", y = "mpg"), list(layer("point"))))
render("01_scatter", ggplot(df, aes(disp, mpg)) + geom_point())

## 02 scatter coloured by factor -----------------------------------------
save("02_scatter_colour", spec(df, list(x = "disp", y = "mpg", colour = "cyl"), list(layer("point"))))
render("02_scatter_colour", ggplot(df, aes(disp, mpg, colour = cyl)) + geom_point())

## 03 bar (count, discrete x) --------------------------------------------
save("03_bar", spec(df, list(x = "cyl"), list(layer("bar"))))
render("03_bar", ggplot(df, aes(cyl)) + geom_bar())

## 04 bar dodged by fill --------------------------------------------------
save("04_bar_dodge", spec(df, list(x = "cyl", fill = "am"), list(layer("bar", position = list(kind = "dodge", width = 0.9)))))
render("04_bar_dodge", ggplot(df, aes(cyl, fill = am)) + geom_bar(position = "dodge"))

## 05 histogram -----------------------------------------------------------
save("05_hist", spec(df, list(x = "mpg"), list(layer("histogram"))))
render("05_hist", ggplot(df, aes(mpg)) + geom_histogram())

## 06 boxplot (continuous y, discrete x) ----------------------------------
save("06_boxplot", spec(df, list(x = "cyl", y = "mpg"), list(layer("boxplot"))))
render("06_boxplot", ggplot(df, aes(cyl, mpg)) + geom_boxplot())

## 07 line grouped by factor ----------------------------------------------
ds <- data.frame(
  x = rep(1:10, each = 2),
  y = c(cumsum(rnorm(10)) + 5, cumsum(rnorm(10, mean = 0.5)) + 2),
  g = rep(c("a", "b"), 10)
)
save("07_line", spec(ds, list(x = "x", y = "y", colour = "g"), list(layer("line"))))
render("07_line", ggplot(ds, aes(x, y, colour = g)) + geom_line())

## 09 boxplot with outlier/median/box colour overrides ------------------------
save("09_box_custom", spec(df, list(x = "cyl", y = "mpg"), list(layer("boxplot", args = list(
  `outlier.colour` = "red", `median.colour` = "blue", `median.linewidth` = 1.5,
  `box.colour` = "#00AA00", notch = "TRUE", outlier.shape = 17)))))
render("09_box_custom", ggplot(df, aes(cyl, mpg)) + geom_boxplot(
  outlier.colour = "red", median.colour = "blue", median.linewidth = 1.5,
  box.colour = "#00AA00", notch = TRUE, outlier.shape = 17))

## 10 boxplot varwidth + whisker/staple/box linewidth -------------------------
save("10_box_vw", spec(df, list(x = "cyl", y = "mpg"), list(layer("boxplot", args = list(
  varwidth = "TRUE", `box.linewidth` = 1.2, `whisker.colour` = "#770000",
  `staple.colour` = "#000077", `outlier.size` = 3)))))
render("10_box_vw", ggplot(df, aes(cyl, mpg)) + geom_boxplot(
  varwidth = TRUE, box.linewidth = 1.2, whisker.colour = "#770000",
  staple.colour = "#000077", outlier.size = 3))

## 08 col with labels ------------------------------------------------------
ds2 <- data.frame(g = c("a", "b", "c"), v = c(3, 7, 5))
save("08_col", spec(ds2, list(x = "g", y = "v"), list(layer("col")),
                    labels = list(title = "Counts", x = "group", y = "value")))
render("08_col", ggplot(ds2, aes(g, v)) + geom_col() + labs(x = "group", y = "value", title = "Counts"))

## 11 log10 y --------------------------------------------------------------
save("11_log10y", spec(df, list(x = "disp", y = "mpg"), list(layer("point")),
     scales = list(y = list(kind = "continuous", transform = list(kind = "log10")))))
render("11_log10y", ggplot(df, aes(disp, mpg)) + geom_point() + scale_y_log10())

## 12 log10 x --------------------------------------------------------------
save("12_log10x", spec(df, list(x = "disp", y = "mpg"), list(layer("point")),
     scales = list(x = list(kind = "continuous", transform = list(kind = "log10")))))
render("12_log10x", ggplot(df, aes(disp, mpg)) + geom_point() + scale_x_log10())

## 13 reverse y ------------------------------------------------------------
save("13_reversy", spec(df, list(x = "disp", y = "mpg"), list(layer("point")),
     scales = list(y = list(kind = "continuous", transform = list(kind = "reverse")))))
render("13_reversy", ggplot(df, aes(disp, mpg)) + geom_point() + scale_y_reverse())

## 14 sqrt y ---------------------------------------------------------------
save("14_sqrty", spec(df, list(x = "disp", y = "mpg"), list(layer("point")),
     scales = list(y = list(kind = "continuous", transform = list(kind = "sqrt")))))
render("14_sqrty", ggplot(df, aes(disp, mpg)) + geom_point() + scale_y_sqrt())

## 15 log10 y with censoring limits ---------------------------------------
save("15_log10lim", spec(df, list(x = "disp", y = "mpg"), list(layer("point")),
     scales = list(y = list(kind = "continuous", transform = list(kind = "log10"),
                             limits = list(15, 30)))))
render("15_log10lim", ggplot(df, aes(disp, mpg)) + geom_point() + scale_y_log10(limits = c(15, 30)))

## 16 histogram binwidth ---------------------------------------------------
save("16_hist_bw", spec(df, list(x = "mpg"), list(layer("histogram", args = list(binwidth = 3)))))
render("16_hist_bw", ggplot(df, aes(mpg)) + geom_histogram(binwidth = 3))

## 17 histogram boundary ---------------------------------------------------
save("17_hist_bound", spec(df, list(x = "mpg"), list(layer("histogram", args = list(bins = 10, boundary = 0)))))
render("17_hist_bound", ggplot(df, aes(mpg)) + geom_histogram(bins = 10, boundary = 0))

## 18 histogram center -----------------------------------------------------
save("18_hist_center", spec(df, list(x = "mpg"), list(layer("histogram", args = list(bins = 10, center = 10)))))
render("18_hist_center", ggplot(df, aes(mpg)) + geom_histogram(bins = 10, center = 10))

## 19 histogram explicit bins ---------------------------------------------
save("19_hist_bins", spec(df, list(x = "mpg"), list(layer("histogram", args = list(bins = 8)))))
render("19_hist_bins", ggplot(df, aes(mpg)) + geom_histogram(bins = 8))

## 20 scatter with shape mapping (default discrete shape seq) --------------
save("20_shape", spec(df, list(x = "disp", y = "mpg", shape = "cyl"), list(layer("point"))))
render("20_shape", ggplot(df, aes(disp, mpg, shape = cyl)) + geom_point())

## 21 point params: shape/fill/size/stroke ---------------------------------
save("21_point_params", spec(df, list(x = "disp", y = "mpg"), list(layer("point",
  args = list(shape = 21, fill = "cyan", colour = "red", size = 3, stroke = 1.5)))))
render("21_point_params", ggplot(df, aes(disp, mpg)) + geom_point(
  shape = 21, fill = "cyan", colour = "red", size = 3, stroke = 1.5))

## 22 hollow shape (pch 1) --------------------------------------------------
save("22_shape1", spec(df, list(x = "disp", y = "mpg"), list(layer("point",
  args = list(shape = 1)))))
render("22_shape1", ggplot(df, aes(disp, mpg)) + geom_point(shape = 1))

## 23 step ------------------------------------------------------------
save("23_step", spec(ds, list(x = "x", y = "y", colour = "g"), list(layer("step"))))
render("23_step", ggplot(ds, aes(x, y, colour = g)) + geom_step())

## 24 freqpoly --------------------------------------------------------
save("24_freqpoly", spec(df, list(x = "mpg"), list(layer("freqpoly", args = list(bins = 10)))))
render("24_freqpoly", ggplot(df, aes(mpg)) + geom_freqpoly(bins = 10))

## 25 hline + vline over a scatter ------------------------------------
hp <- df; hp$hp_ref <- mean(hp$mpg)
save("25_hline", spec(hp, list(x = "disp", y = "mpg"),
     list(layer("point"), layer("hline", args = list(yintercept = mean(hp$mpg))))))
render("25_hline", ggplot(hp, aes(disp, mpg)) + geom_point() + geom_hline(yintercept = mean(hp$mpg), colour = "red"))

## 26 vline -----------------------------------------------------------
save("26_vline", spec(hp, list(x = "disp", y = "mpg"),
     list(layer("point"), layer("vline", args = list(xintercept = mean(hp$disp))))))
render("26_vline", ggplot(hp, aes(disp, mpg)) + geom_point() + geom_vline(xintercept = mean(hp$disp), colour = "blue"))

## 27 stacked bar ----------------------------------------------------------
save("27_stack", spec(df, list(x = "cyl", fill = "vs"),
     list(layer("bar", position = list(kind = "stack")))))
render("27_stack", ggplot(df, aes(cyl, fill = vs)) + geom_bar(position = "stack"))

## 28 text labels ----------------------------------------------------------
dtext <- data.frame(x = c(50, 150, 250), y = c(30, 20, 10), lab = c("low", "mid", "high"))
save("28_text", spec(dtext, list(x = "x", y = "y", label = "lab"), list(layer("text"))))
render("28_text", ggplot(dtext, aes(x, y, label = lab)) + geom_text())

## 29 area -----------------------------------------------------------------
darea <- data.frame(x = 1:8, y = c(3, 5, 4, 6, 5, 7, 6, 8))
save("29_area", spec(darea, list(x = "x", y = "y"), list(layer("area"))))
render("29_area", ggplot(darea, aes(x, y)) + geom_area())

## 30 errorbar (discrete x isolates the errorbar geometry) ------------------
derr <- data.frame(x = factor(1:4), ymin = c(1, 2, 3, 4), ymax = c(3, 4, 5, 6))
save("30_errorbar", spec(derr, list(x = "x", ymin = "ymin", ymax = "ymax"), list(layer("errorbar"))))
render("30_errorbar", ggplot(derr, aes(x, ymin = ymin, ymax = ymax)) + geom_errorbar())

## 31 ribbon ----------------------------------------------------------------
drib <- data.frame(x = 1:6, ymin = c(1, 2, 1, 3, 2, 4), ymax = c(3, 4, 3, 5, 4, 6))
save("31_ribbon", spec(drib, list(x = "x", ymin = "ymin", ymax = "ymax"), list(layer("ribbon"))))
render("31_ribbon", ggplot(drib, aes(x, ymin = ymin, ymax = ymax)) + geom_ribbon())

## 32 size aes (area scale) --------------------------------------------------
save("32_size", spec(df, list(x = "disp", y = "mpg", size = "hp"), list(layer("point"))))
render("32_size", ggplot(df, aes(disp, mpg, size = hp)) + geom_point())

## 33 alpha aes ---------------------------------------------------------------
save("33_alpha", spec(df, list(x = "disp", y = "mpg", alpha = "hp"), list(layer("point"))))
render("33_alpha", ggplot(df, aes(disp, mpg, alpha = hp)) + geom_point())

## 34 n.breaks ----------------------------------------------------------------
save("34_nbreaks", spec(df, list(x = "disp", y = "mpg"), list(layer("point")),
     scales = list(y = list(kind = "continuous", n_breaks = 10))))
render("34_nbreaks", ggplot(df, aes(disp, mpg)) + geom_point() + scale_y_continuous(n.breaks = 10))

## 35 boxplot coef -------------------------------------------------------------
save("35_box_coef", spec(df, list(x = "cyl", y = "mpg"), list(layer("boxplot", args = list(coef = 0.5)))))
render("35_box_coef", ggplot(df, aes(cyl, mpg)) + geom_boxplot(coef = 0.5))

## 36 linetype mapping -------------------------------------------------------
dlt <- data.frame(x = rep(1:6, each = 1), y = 1:6)
dlt <- transform(dlt, x = rep(1:10, 3), y = rep(1:10, 3), l = rep(c("a", "b", "c"), each = 10))
save("36_linetype", spec(dlt, list(x = "x", y = "y", linetype = "l"), list(layer("line"))))
render("36_linetype", ggplot(dlt, aes(x, y, linetype = l)) + geom_line())

## 37 continuous colour (Lab gradient) ---------------------------------------
save("37_colour_cont", spec(df, list(x = "disp", y = "mpg", colour = "hp"), list(layer("point"))))
render("37_colour_cont", ggplot(df, aes(disp, mpg, colour = hp)) + geom_point())

## 38 continuous colour with gradient scale -----------------------------------
save("38_gradient", spec(df, list(x = "disp", y = "mpg", colour = "hp"), list(layer("point")),
     scales = list(colour = list(kind = "gradient", low = "#132B43", high = "#56B1F7"))))
render("38_gradient", ggplot(df, aes(disp, mpg, colour = hp)) + geom_point() + scale_colour_gradient())

cat("exported", length(list.files("specs")), "specs + refs\n")
