use potoo_linotte_interpreter::execute;

fn assert_book(source: &str, expected: &str) {
    let mut output = Vec::new();
    execute(source, &mut output).unwrap();
    assert_eq!(String::from_utf8(output).unwrap(), expected);
}

#[test]
fn official_314_bonjour_example() {
    assert_book("affiche \"Bonjour !\"", "Bonjour !\n");
}

#[test]
fn official_314_assignment_example() {
    assert_book("a est un texte\na prend \"ronan\"\naffiche a", "ronan\n");
}

#[test]
fn official_314_named_function_parameters_example() {
    assert_book(
        "affiche somme(1,2)\nsomme : a, b\n    retourne a + b",
        "3\n",
    );
}

#[test]
fn paragraph_actor_alias_rejects_a_value_of_the_wrong_role() {
    let source = "control :\n    *age est un nombre\n    retourne age\ntexte est un texte valant \"Jade\"\naffiche control(texte)";
    let diagnostic = execute(source, &mut Vec::new()).unwrap_err();

    assert!(diagnostic.message.contains("rôle"));
}

#[test]
fn annihile_removes_the_actor_from_its_scope() {
    let source = "texte est un texte valant \"disparaître\"\nannihile texte\naffiche texte";
    let diagnostic = execute(source, &mut Vec::new()).unwrap_err();

    assert!(diagnostic.message.contains("inconnu"));
}

#[test]
fn official_314_paragraph_actor_parameters_example() {
    assert_book(
        "affiche somme(1,2)\nsomme :\n    * a est un nombre\n    * b est un nombre\n    retourne a + b",
        "3\n",
    );
}

#[test]
fn official_314_simple_add_and_subtract_example() {
    assert_book(
        "a est un nombre valant 10\na += 13\na -= 3\naffiche a",
        "20\n",
    );
}

#[test]
fn official_314_collection_iteration_example() {
    assert_book(
        "liste est un casier de texte valant \"Janvier\", \"Fevrier\"\nvaleur est un texte\npour chaque valeur de liste affiche valeur",
        "Janvier\nFevrier\n",
    );
}

#[test]
fn custom_successor_loop_rereads_its_lower_bound_actor() {
    assert_book(
        "début est un nombre valant 1\nfin est un nombre valant 5\ncompteur est un nombre\npour compteur de début à fin suivant compteur + 1\n    affiche compteur\n    si compteur = 1\n        début prend 4\n    ferme\nferme",
        "1\n",
    );
}

#[test]
fn official_314_species_case_insensitivity_example() {
    assert_book(
        "espèces\n    espèce Formsp\nDémonstration :\n    Messager est un FormSp\n        affiche \"ok\"",
        "ok\n",
    );
}

#[test]
fn official_314_text_prototype_examples() {
    assert_book(
        "chaine est un texte valant \"Bonjour\"\naffiche chaine.inverser()\nchaine2 est un texte valant \"Bonjour la terre et les terriens !\"\naffiche chaine2.remplacer(\"terr\", \"lun\")\nchaine3 est un texte valant \"bonjour la   TERRE et les terriens\"\naffiche chaine3.capitaliser()",
        "ruojnoB\nBonjour la lune et les luniens !\nBonjour La   Terre Et Les Terriens\n",
    );
}

#[test]
fn official_314_text_prototype_index_and_case_examples() {
    assert_book(
        "chaine est un texte valant \"Hello\"\nchaine2 est un texte valant \"Bonjour la terre et les terriens !\"\naffiche chaine.majuscule()\naffiche chaine.minuscule()\naffiche chaine.taille()\naffiche chaine2.position(0,\"terr\")\naffiche chaine2.position(0,\"lun\")\naffiche chaine2.position(13,\"terr\")\naffiche \"{chaine2.extraire(0,7)} !\"",
        "HELLO\nhello\n5\n11\n-1\n24\nBonjour !\n",
    );
}

#[test]
fn official_314_version_example() {
    assert_book("affiche \"{version}\"", "π\n");
}

#[test]
fn official_314_anonymous_text_method_example() {
    assert_book("affiche \"linotte\".majuscule()", "LINOTTE\n");
}

#[test]
fn official_314_implicit_actor_creation_example() {
    assert_book(
        "nom prend \"Ronan\"\nprenom prend \"Magali\"\naffiche nom + prenom",
        "RonanMagali\n",
    );
}

#[test]
fn official_314_species_with_collection_attributes_example() {
    assert_book(
        "espèces\n    a est un casier de textes\n    c est un casier de nombres\n    d est un casier de casiers\n    f est un casier de rectangle\n    espèce e contient a,c,d,f\nDémonstration :\n    x est un e\n        ajoute \"a\" dans a de x\n        ajoute \"b\" dans a de x\n        ajoute 1 dans c de x\n        ajoute 2 dans c de x\n        affiche x",
        "a\nb\n1\n2\n",
    );
}

#[test]
fn official_314_unicode_book_example() {
    assert_book(
        "principale :\n    phrase est un texte valant µ\"0022\" + \"Voici mon texte !\" + µ\"0022\"\n    phrase !\n    µ\"06A5\" !",
        "\"Voici mon texte !\"\n\u{06A5}\n",
    );
}

#[test]
fn ascii_of_an_empty_text_uses_the_java_space_default() {
    assert_book("affiche ascii(\"\")", "32\n");
}

#[test]
fn ascii_preserves_an_isolated_surrogate_from_chr() {
    assert_book("affiche ascii(chr(55296))", "55296\n");
}

#[test]
fn chr_uses_java_integer_cast_wrapping_for_large_numbers() {
    assert_book("affiche ascii(chr(18446744073709551616))", "0\n");
}

#[test]
fn ascii_preserves_an_isolated_surrogate_from_unicode() {
    assert_book("affiche ascii(unicode(\"D800\"))", "55296\n");
}

#[test]
fn concatenating_surrogate_halves_forms_the_unicode_scalar() {
    assert_book("affiche chr(55357) + chr(56832)", "😀\n");
}

#[test]
fn text_iteration_uses_utf16_code_units() {
    assert_book(
        "texte est un texte valant \"😀\"\npour chaque texte\n    affiche ascii(joker)\nferme",
        "55357\n56832\n",
    );
}

#[test]
fn text_indexing_selects_a_utf16_code_unit() {
    assert_book(
        "texte est un texte valant \"😀\"\naffiche ascii(texte{1})",
        "56832\n",
    );
}

#[test]
fn text_extraction_keeps_an_isolated_surrogate() {
    assert_book("affiche ascii(\"😀\".extraire(0,1))", "55357\n");
}

#[test]
fn text_replacement_can_match_a_single_surrogate_unit() {
    assert_book(
        "affiche ascii(\"😀\".remplacer(chr(55357), \"x\"))",
        "120\n",
    );
}

#[test]
fn evaluated_text_keeps_an_isolated_surrogate_result() {
    assert_book(
        "texte est un texte valant \"chr(55296)\"\névalue texte\naffiche ascii(texte)",
        "55296\n",
    );
}

#[test]
fn text_methods_keep_unpaired_surrogates_while_changing_case() {
    assert_book("affiche ascii(chr(55296).majuscule())", "55296\n");
}

#[test]
fn position_uses_java_simple_utf16_case_folding_without_expansions() {
    assert_book("affiche \"İ\".position(0, \"i̇\")", "-1\n");
}

#[test]
fn position_clamps_a_negative_start_like_java_index_of() {
    assert_book("affiche \"abc\".position(-1, \"a\")", "0\n");
}

#[test]
fn capitaliser_uses_java_ascii_whitespace_splitting() {
    assert_book("affiche \" ABC\".capitaliser()", " abc\n");
}

#[test]
fn position_search_start_counts_original_utf16_units_before_case_mapping() {
    assert_book(
        "texte est un texte valant \"İx\"\naffiche texte.position(1, \"x\")",
        "1\n",
    );
}

#[test]
fn unicode_uses_java_int_to_char_truncation() {
    assert_book("affiche unicode(\"1F600\")", "\u{F600}\n");
}

#[test]
fn chr_truncates_fractional_numbers_toward_zero_before_casting() {
    assert_book(
        "affiche ascii(chr(65.9))\naffiche ascii(chr(-65.9))",
        "65\n65471\n",
    );
}
