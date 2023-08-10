CC=clang

candy_runtime.a: candy_runtime.o candy_builtin.o
	ar rcs $@ $^

%.o: %.c %.h

.PHONY: clean
clean:
	rm -f *.o *.a
