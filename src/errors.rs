error_chain! {
    types {
    }

    links {
    }

    foreign_links {
        HyperError(::hyper::Error);
        Io(::std::io::Error);
    }
}
