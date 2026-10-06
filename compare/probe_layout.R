# probe_layout.R — dump ggplot2 gtable cell widths/heights (px @72dpi) per plot.
# Layout constants in layout.rs must match these gutters.
suppressMessages(library(ggplot2))
suppressMessages(library(grid))

dump <- function(tag, p) {
  png(tempfile(), width = 720, height = 480, res = 72, type = "cairo")
  print(p)
  gt <- ggplotGrob(p)
  wp <- convertUnit(gt$widths, "inches", valueOnly = TRUE) * 72
  hp <- convertUnit(gt$heights, "inches", valueOnly = TRUE) * 72
  # nonzero entries only, with names
  wn <- which(wp > 0.005); hn <- which(hp > 0.005)
  cat(sprintf("%-12s W:", tag), paste(sprintf("%.2f[%s]", wp[wn], sapply(wn, function(i) paste(gt$layout$name[gt$layout$l <= i & gt$layout$r >= i], collapse = ","))), collapse = "  "), "\n")
  cat(sprintf("%-12s H:", tag), paste(sprintf("%.2f[%s]", hp[hn], sapply(hn, function(i) paste(gt$layout$name[gt$layout$t <= i & gt$layout$b >= i], collapse = ","))), collapse = "  "), "\n")
  invisible(dev.off())
}

df <- mtcars; df$cyl <- factor(df$cyl); df$am <- factor(df$am)
dump("scatter", ggplot(df, aes(disp, mpg)) + geom_point())
dump("scatter2", ggplot(df[1:3,], aes(factor(c(cyl[1:3])), c(mpg[1:3]))) + geom_point())
dump("bar", ggplot(df, aes(cyl)) + geom_bar())
dump("dodge", ggplot(df, aes(cyl, fill = am)) + geom_bar(position = "dodge"))
dump("hist", ggplot(df, aes(mpg)) + geom_histogram())
dump("box", ggplot(df, aes(cyl, mpg)) + geom_boxplot())
dump("col", ggplot(df, aes(disp, mpg)) + geom_point() + labs(title = "T"))
dump("title", ggplot(df, aes(disp, mpg)) + geom_point() + labs(title = "Counts subtitle test"))
