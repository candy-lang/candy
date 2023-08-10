CC=clang

candy_rt.a: candy_rt.o candy_builtin.o
	ar rcs $@ $^

%.o: %.c %.h

.PHONY: clean
clean:
	rm -f *.o *.a
