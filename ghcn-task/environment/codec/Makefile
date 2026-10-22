# Builds ./codec. The grader runs `make` in a copy of this directory and
# then uses ./codec (plus anything under ./model/, if present).
CXX ?= g++
CXXFLAGS ?= -O2 -Wall
LDLIBS = -lz

codec: codec.cpp
	$(CXX) $(CXXFLAGS) -o $@ $< $(LDLIBS)

clean:
	rm -f codec

.PHONY: clean
