PAPER = paper

.PHONY: all clean veryclean view

all: $(PAPER).pdf

$(PAPER).pdf: $(PAPER).tex references.bib
	pdflatex -interaction=nonstopmode -halt-on-error $(PAPER).tex
	bibtex $(PAPER)
	pdflatex -interaction=nonstopmode -halt-on-error $(PAPER).tex
	pdflatex -interaction=nonstopmode -halt-on-error $(PAPER).tex

view: $(PAPER).pdf
	@which xdg-open >/dev/null 2>&1 && xdg-open $(PAPER).pdf || \
	 echo "Open $(PAPER).pdf with your PDF viewer."

clean:
	rm -f $(PAPER).aux $(PAPER).log $(PAPER).out $(PAPER).bbl \
	      $(PAPER).blg $(PAPER).toc $(PAPER).lof $(PAPER).lot \
	      $(PAPER).fdb_latexmk $(PAPER).fls $(PAPER).synctex.gz

veryclean: clean
	rm -f $(PAPER).pdf
