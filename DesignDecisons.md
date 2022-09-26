# Software design decisions

This file arose as I wanted somewhere for notes on _why_ I make changes - a bit like Architecture Decision Records, but it's a bit grand to call them Architecture :)

Mostly as right now (Sep 2022) I'm reversing an original decision and without having a pair to talk to, making notes here is useful for me!

## Sep 2022 - stopping using Value for Toxicity Calculators

Originally I built this scanner a bit too generically.  You'd think after decades of preaching "YAGNI - You Ain't Gonna Need It" I'd have learned better, but no...

So the scanner used to have these fairly generic `ToxicityIndicatorCalculator` structs, which have two methods:

* `calculate` which is the heart of the calculator, it is a pure-ish function that returns a JSON `Value` for the calculator - e.g. for the Lines of Code one it returns a set of code lines metrics - this is called for each file/dir scanned, and the returned `Value` is added to the `data` for each file/dir
* `metadata` which is called at the end, to store any metadata that the calculator generates and needs to be saved.  This also used a `Value`

This seemed like a good idea at the time - nice to have side-effect-free functions, and the `Value` returns meant no coupling between the calculators and the rest of the app.  

But, once I moved the Explorer to TypeScript, I had to re-build the types used in the Scanner in TypeScript - and I realised that really the use of `Value` meant I was bypassing the type system.  And I only have 3 Indicators! So why all this effort for generic behaviour?  There's no point making the Rust flexible when the TypeScript isn't!

(I think when I started I had no idea how many indicators I would want, and I could see it being some kind of place I could plug in language-specific tools... like I said, YAGNI should have applied)

So, I want to go to Value-less code.  I can see two options:

1. Instead of `calculate` returning a `Value` I make it generic, so `calculate<T>` returns a `T`
2. Make it a visitor instead. `calculate` takes a mutable `FileTreeNode` parameter, it changes the data it needs to.

I'm going for option `2` - it feels a lot simpler.

The only downside here is - this made some unit tests harder. The more-generic code could be tested by throwing fake `Value` objects around for tests - the new code only accepts 'real' types.  This is probably good overall, as it means the tests are closer to reality.  But some things aren't well tested, except in end-to-end tests.