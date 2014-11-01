SRC_FILE=imap.ump

all: clean graph

graph:
	java -jar umple.jar -g GvClassDiagram ${SRC_FILE}
	dot -Tsvg imapcd.gv -o imapgraph.svg

clean:
	rm -rf imapcd.gv imapgraph.svg
