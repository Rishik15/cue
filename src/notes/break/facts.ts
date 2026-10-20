/**
 * Purpose: The one-line facts the break screen shows, one picked at random for each break.
 * Contents: FACTS — the lines; pickFact — one of them.
 */
export const FACTS = [
  "Octopuses have three hearts and blue blood.",
  "A group of crows is called a murder, and they remember faces.",
  "Honey from a sealed jar can stay good for thousands of years.",
  "Sea otters hold hands while they sleep so they do not drift apart.",
  "A day on Venus is longer than its year.",
  "Wombats produce cube-shaped droppings.",
  "Bananas are slightly radioactive because of their potassium.",
  "Hummingbirds are the only birds that can fly backwards.",
  "Lightning strikes the Earth about a hundred times every second.",
  "A snail can sleep for up to three years.",
  "Sharks are older than trees.",
  "The heart of a blue whale is about the size of a small car.",
  "Cows have best friends and get stressed when they are apart.",
  "Water can boil and freeze at the same time at the triple point.",
  "A flamingo is pink because of the shrimp and algae it eats.",
  "Starfish have no brain and no blood.",
  "Sound travels about four times faster in water than in air.",
  "Elephants can recognise themselves in a mirror.",
  "A teaspoon of a neutron star would weigh billions of tonnes.",
  "Penguins propose to a mate with a carefully chosen pebble.",
  "Your stomach lining replaces itself every few days.",
  "Koalas have fingerprints almost identical to human ones.",
  "Jellyfish have survived for more than 500 million years.",
  "Owls cannot move their eyes, so they turn their heads instead.",
  "There are more stars in the sky than grains of sand on Earth's beaches.",
  "Ants never sleep and have no lungs.",
  "A cloud can weigh as much as a hundred elephants.",
  "Goats have rectangular pupils that give them a wide view.",
  "The human body glows faintly, but too dimly for the eye to see.",
  "Dolphins call each other by name with signature whistles.",
];

export const pickFact = () => FACTS[Math.floor(Math.random() * FACTS.length)] ?? "";
