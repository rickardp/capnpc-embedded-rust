@0xd9e2dd9f7d7a5b0f;
struct Person {
  name  @0 :Text;
  email @1 :Text;
}
struct AddressBook {
  people @0 :List(Person);
}
