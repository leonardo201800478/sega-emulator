#pragma once

// Minimal, project-authored compatibility surface for the pinned Ares SN76489
// component. This is not part of Ares and is not used by the production core.
#include <cstddef>
#include <cstdint>
#include <initializer_list>

namespace ares {

template <unsigned Width>
struct bitfield {
  static constexpr std::uint32_t mask = (std::uint32_t{1} << Width) - 1;
  std::uint32_t value = 0;

  constexpr bitfield() = default;
  constexpr bitfield(std::uint32_t initial) : value(initial & mask) {}
  constexpr bitfield(const bitfield&) = default;
  auto operator=(const bitfield&) -> bitfield& = default;
  auto operator=(std::uint32_t next) -> bitfield& {
    value = next & mask;
    return *this;
  }
  constexpr operator std::uint32_t() const { return value; }
  constexpr explicit operator bool() const { return value != 0; }
  auto operator--() -> bitfield& {
    value = (value - 1) & mask;
    return *this;
  }
  auto operator^=(std::uint32_t rhs) -> bitfield& {
    value = (value ^ rhs) & mask;
    return *this;
  }

  struct slice {
    bitfield& owner;
    unsigned first;
    unsigned last;
    operator std::uint32_t() const {
      const unsigned width = last - first + 1;
      const std::uint32_t field_mask = (std::uint32_t{1} << width) - 1;
      return (owner.value >> first) & field_mask;
    }
    auto operator=(std::uint32_t next) -> slice& {
      const unsigned width = last - first + 1;
      const std::uint32_t field_mask = ((std::uint32_t{1} << width) - 1) << first;
      owner.value = (owner.value & ~field_mask) | ((next << first) & field_mask);
      return *this;
    }
  };

  constexpr auto bit(unsigned index) const -> bool {
    return ((value >> index) & 1U) != 0;
  }
  constexpr auto bit(unsigned first, unsigned last) const -> std::uint32_t {
    const unsigned width = last - first + 1;
    return (value >> first) & ((std::uint32_t{1} << width) - 1);
  }
  auto bit(unsigned first, unsigned last) -> slice {
    return slice{*this, first, last};
  }
};

using n1 = bitfield<1>;
using n2 = bitfield<2>;
using n4 = bitfield<4>;
using n8 = bitfield<8>;
using n10 = bitfield<10>;
using n16 = bitfield<16>;

template <typename T>
struct array;

template <typename T, std::size_t N>
struct array<T[N]> {
  T values[N]{};
  array() = default;
  array(std::initializer_list<T> init) {
    std::size_t i = 0;
    for (const auto& item : init) {
      if (i == N) break;
      values[i++] = item;
    }
  }
  auto operator[](std::size_t index) -> T& { return values[index]; }
  auto operator[](std::size_t index) const -> const T& { return values[index]; }
};

struct serializer {
  template <typename T>
  auto operator()(T&) -> void {}
};

}  // namespace ares
