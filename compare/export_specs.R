# export_specs.R — writes jplot spec JSON + ggplot2 reference PNGs, same data.
# Run: Rscript export_specs.R  (in this directory)
# Refs rendered at 72 dpi: width=720/72 in, height=480/72 in.
suppressMessages(library(ggplot2))

dir.create("specs", showWarnings = FALSE)
dir.create("refs", showWarnings = FALSE)
dir.create("ours", showWarnings = FALSE)

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
    boxplot = list(kind = "boxplot")
  )
  list(geom = g, stat = stat, position = position,
       mapping = if (is.null(mapping)) list(map = list()) else list(map = mapping),
       args = args, inherit_aes = TRUE)
}

save <- function(name, sp) {
  jsonlite::write_json(sp, file.path("specs", paste0(name, ".json")), auto_unbox = TRUE, digits = 10, null = "null", empty_object = TRUE)
}
render <- function(name, p) {
  png(file.path("refs", paste0(name, ".png")), width = 720, height = 480, res = 72, type = "cairo")
  print(p)
  invisible(dev.off())
}

suppressMessages(library(jsonlite))

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

## 08 col with labels ------------------------------------------------------
ds2 <- data.frame(g = c("a", "b", "c"), v = c(3, 7, 5))
save("08_col", spec(ds2, list(x = "g", y = "v"), list(layer("col")),
                    labels = list(title = "Counts", x = "group", y = "value")))
render("08_col", ggplot(ds2, aes(g, v)) + geom_col() + labs(x = "group", y = "value", title = "Counts"))

cat("exported", length(list.files("specs")), "specs + refs\n")
